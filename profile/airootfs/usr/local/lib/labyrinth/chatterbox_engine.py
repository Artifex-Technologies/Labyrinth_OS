"""Chatterbox Turbo (Resemble AI, MIT) from its official ONNX files, for Sarina's voice. Used by labyrinth-voiced (the voice service) and
scripts/voice/chatterbox_onnx_test.py. Needs onnxruntime (onnxruntime-gpu for the graphics card), tokenizers, numpy, soundfile, scipy.

The four graphs: a speech encoder (turns a reference recording into conditioning), token embeddings, an autoregressive language model with a
KV cache (writes speech tokens), and a conditional decoder (speech tokens to a 24 kHz waveform). See docs/SARINA_VOICE.md for what the short
example in the model card leaves out: exactly one <|endoftext|> after the text, and real sampling instead of always taking the top token.
"""
import os
import time
from math import gcd

import numpy as np
import onnxruntime as ort
import soundfile as sf
from scipy.signal import resample_poly
from tokenizers import Tokenizer

SR = 24000
START, STOP, SILENCE, EOT = 6561, 6562, 4299, 50256
KV_HEADS, HEAD_DIM = 16, 64


def load_audio(path):
    """Mono float32 audio at 24 kHz, shape (1, samples)."""
    a, sr = sf.read(path, dtype="float32", always_2d=False)
    if a.ndim > 1:
        a = a.mean(axis=1)
    if sr != SR:
        g = gcd(sr, SR)
        a = resample_poly(a, SR // g, sr // g).astype(np.float32)
    return a[np.newaxis, :]


def sample(logits, rng, temperature=0.8, top_k=1000, top_p=0.95):
    """Temperature, top-k and top-p sampling (Chatterbox's own defaults)."""
    l = logits / temperature
    if top_k < len(l):
        kth = np.partition(l, -top_k)[-top_k]
        l = np.where(l < kth, -np.inf, l)
    p = np.exp(l - np.max(l))
    p /= p.sum()
    order = np.argsort(-p)
    cum = np.cumsum(p[order])
    keep = order[: int(np.searchsorted(cum, top_p)) + 1]
    q = np.zeros_like(p)
    q[keep] = p[keep]
    q /= q.sum()
    return int(rng.choice(len(q), p=q))


def trim_silence(wav, sr=SR, keep=0.2, thresh=1e-3):
    """Cuts digital silence from the end (the model likes to end with a long pause), keeping a short tail."""
    idx = np.nonzero(np.abs(wav) > thresh)[0]
    if len(idx) == 0:
        return wav
    return wav[: min(len(wav), idx[-1] + int(keep * sr))]


def suffix(dtype):
    return "" if dtype == "fp32" else "_quantized" if dtype == "q8" else "_" + dtype


class Chatterbox:
    def __init__(self, directory, dtype="fp16", gpu=True, decoder_dtype="fp32"):
        self.dir, self.dtype = os.path.expanduser(directory), dtype
        providers = ["CPUExecutionProvider"]
        if gpu:
            if hasattr(ort, "preload_dlls"):
                ort.preload_dlls()
            # (tried "allocate exactly what is asked for": the attention kernels then fail; the default arena works)
            providers = ["CUDAExecutionProvider", "CPUExecutionProvider"]
        self.providers = [p if isinstance(p, str) else p[0] for p in providers]
        t = time.time()

        def session(name, dt=None):
            return ort.InferenceSession(os.path.join(self.dir, "onnx", name + suffix(dt or dtype) + ".onnx"), providers=providers)

        # The decoder in half precision overflows for some sentences (the audio comes out as NaN, which plays as a loud pop), so it stays in
        # full precision (about 0.4 GB more) while the rest runs in the smaller type.
        self.enc, self.emb, self.lm = session("speech_encoder"), session("embed_tokens"), session("language_model")
        self.dec = session("conditional_decoder", decoder_dtype if dtype != "fp32" else dtype)
        self.tok = Tokenizer.from_file(os.path.join(self.dir, "tokenizer.json"))
        self.kv_names = [x.name for x in self.lm.get_inputs() if "past_key_values" in x.name]
        self.kv_dtype = [np.float16 if x.type == "tensor(float16)" else np.float32 for x in self.lm.get_inputs() if "past_key_values" in x.name]
        self.load_seconds = time.time() - t
        self.voice = None

    def set_voice(self, reference_wav):
        """Works out the conditioning for the voice to copy (about 10 seconds of clean speech); done once, then every sentence reuses it.
        The half-precision speech encoder gives NaN in some of its features (the whole sentence then comes out as noise), so this one step runs in full precision on the
        processor (a few seconds, once) when the full-precision file is there; whatever NaN remains is zeroed."""
        audio = load_audio(reference_wav)
        enc = self.enc
        full = os.path.join(self.dir, "onnx", "speech_encoder.onnx")
        if self.dtype != "fp32" and os.path.exists(full):
            enc = ort.InferenceSession(full, providers=["CPUExecutionProvider"])
        cond_emb, prompt_token, speaker_embeddings, speaker_features = enc.run(None, {"audio_values": audio})
        self.voice = (np.nan_to_num(cond_emb), prompt_token, np.nan_to_num(speaker_embeddings), np.nan_to_num(speaker_features))
        if enc is not self.enc:
            del enc
        self.enc = None  # not needed again: frees the graphics card memory it held

    def speak(self, text, rng, max_tokens=700, repetition_penalty=1.2, temperature=0.8):
        """One sentence (or short passage) to a float32 waveform at 24 kHz."""
        assert self.voice is not None, "set_voice first"
        cond_emb, prompt_token, speaker_embeddings, speaker_features = self.voice
        ids = np.array([self.tok.encode(text, add_special_tokens=False).ids + [EOT]], dtype=np.int64)
        tokens = np.array([[START]], dtype=np.int64)
        past, mask, pos = None, None, None
        for i in range(max_tokens):
            embeds = self.emb.run(None, {"input_ids": ids})[0]
            if i == 0:
                embeds = np.concatenate((cond_emb, embeds), axis=1)
                b, n, _ = embeds.shape
                past = {k: np.zeros([b, KV_HEADS, 0, HEAD_DIM], dtype=dt) for k, dt in zip(self.kv_names, self.kv_dtype)}
                mask = np.ones((b, n), dtype=np.int64)
                pos = np.arange(n, dtype=np.int64).reshape(1, -1).repeat(b, axis=0)
            logits, *present = self.lm.run(None, dict(inputs_embeds=embeds, attention_mask=mask, position_ids=pos, **past))
            logits = logits[:, -1, :].astype(np.float32)
            sc = np.take_along_axis(logits, tokens, axis=1)
            sc = np.where(sc < 0, sc * repetition_penalty, sc / repetition_penalty)
            np.put_along_axis(logits, tokens, sc, axis=1)
            ids = np.array([[sample(logits[0], rng, temperature=temperature)]], dtype=np.int64)
            tokens = np.concatenate((tokens, ids), axis=-1)
            if int(ids[0, 0]) == STOP:
                break
            mask = np.concatenate([mask, np.ones((b, 1), dtype=np.int64)], axis=1)
            pos = pos[:, -1:] + 1
            for j, k in enumerate(self.kv_names):
                past[k] = present[j]
        speech = tokens[:, 1:-1]
        speech = np.concatenate([prompt_token, speech, np.full((1, 3), SILENCE, dtype=np.int64)], axis=1)
        wav = self.dec.run(None, dict(speech_tokens=speech, speaker_embeddings=speaker_embeddings, speaker_features=speaker_features))[0].squeeze(axis=0)
        wav = np.asarray(wav, dtype=np.float32)
        if not np.all(np.isfinite(wav)):
            raise ValueError("the voice produced invalid audio")  # the caller tries again with another random draw
        peak = float(np.max(np.abs(wav))) if wav.size else 0.0
        if peak > 0.95:
            wav = wav * (0.95 / peak)  # the decoder can overshoot full scale: scale down instead of letting it clip (clipping crackles)
        return trim_silence(wav)
