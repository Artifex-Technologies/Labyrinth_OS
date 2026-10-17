#!/usr/bin/env python3
"""Try Chatterbox Turbo (MIT, Resemble AI) through its official ONNX export, on the CPU or GPU, from a short reference recording.

  chatterbox_onnx_test.py REF.wav "text to say" OUT.wav [--dtype fp32|fp16|q8|q4|q4f16] [--gpu] [--seed N]

The reference is mono audio (any rate; it is resampled to 24 kHz), about 10 seconds of clean speech of the voice to copy.
The models are read from ~/.local/share/labyrinth/tts/chatterbox/onnx (see docs/SARINA_VOICE.md for how they got there).
This follows Resemble's own example for the ONNX files; it needs only onnxruntime, tokenizers, soundfile, scipy and numpy.
"""
import os
import sys
import time

import numpy as np
import onnxruntime as ort
import soundfile as sf
from scipy.signal import resample_poly
from tokenizers import Tokenizer

DIR = os.path.expanduser(os.environ.get("CHATTERBOX_DIR", "~/.local/share/labyrinth/tts/chatterbox"))
SR = 24000
START, STOP, SILENCE = 6561, 6562, 4299
KV_HEADS, HEAD_DIM = 16, 64


def load_audio(path):
    a, sr = sf.read(path, dtype="float32", always_2d=False)
    if a.ndim > 1:
        a = a.mean(axis=1)
    if sr != SR:
        from math import gcd
        g = gcd(sr, SR)
        a = resample_poly(a, SR // g, sr // g).astype(np.float32)
    return a[np.newaxis, :]


def sample(logits, rng, temperature=0.8, top_k=1000, top_p=0.95):
    """Temperature, top-k and top-p sampling (Chatterbox's own defaults). Always taking the single most likely token, as the
    short ONNX example does, makes the voice stall and leave long silences."""
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


def trim_silence(wav, sr=SR, keep=0.25, thresh=1e-3):
    """Cuts digital silence from the end (the model likes to end with a long pause), keeping a short tail."""
    idx = np.nonzero(np.abs(wav) > thresh)[0]
    if len(idx) == 0:
        return wav
    return wav[: min(len(wav), idx[-1] + int(keep * sr))]


def suffix(dtype):
    return "" if dtype == "fp32" else "_quantized" if dtype == "q8" else "_" + dtype


def main(argv):
    if len(argv) < 4:
        print(__doc__)
        return 2
    ref, text, out = argv[1], argv[2], argv[3]
    dtype = argv[argv.index("--dtype") + 1] if "--dtype" in argv else "fp32"
    providers = ["CUDAExecutionProvider", "CPUExecutionProvider"] if "--gpu" in argv else ["CPUExecutionProvider"]
    seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 1
    rng = np.random.default_rng(seed)
    t0 = time.time()

    def session(name):
        return ort.InferenceSession(os.path.join(DIR, "onnx", name + suffix(dtype) + ".onnx"), providers=providers)

    enc, emb, lm, dec = session("speech_encoder"), session("embed_tokens"), session("language_model"), session("conditional_decoder")
    t_load = time.time() - t0
    tok = Tokenizer.from_file(os.path.join(DIR, "tokenizer.json"))
    # the text tokens plus ONE <|endoftext|> (50256): the embedding graph reads the last token of its first input as the speech-start slot
    input_ids = np.array([tok.encode(text, add_special_tokens=False).ids + [50256]], dtype=np.int64)
    audio = load_audio(ref)
    t1 = time.time()
    cond_emb, prompt_token, speaker_embeddings, speaker_features = enc.run(None, {"audio_values": audio})
    tokens = np.array([[START]], dtype=np.int64)
    ids = input_ids
    past = None
    for i in range(1024):
        embeds = emb.run(None, {"input_ids": ids})[0]
        if i == 0:
            embeds = np.concatenate((cond_emb, embeds), axis=1)
            b, n, _ = embeds.shape
            past = {x.name: np.zeros([b, KV_HEADS, 0, HEAD_DIM], dtype=np.float16 if x.type == "tensor(float16)" else np.float32) for x in lm.get_inputs() if "past_key_values" in x.name}
            mask = np.ones((b, n), dtype=np.int64)
            pos = np.arange(n, dtype=np.int64).reshape(1, -1).repeat(b, axis=0)
        logits, *present = lm.run(None, dict(inputs_embeds=embeds, attention_mask=mask, position_ids=pos, **past))
        logits = logits[:, -1, :].astype(np.float32)
        # repetition penalty 1.2 over the tokens made so far, as in Resemble's example
        seen = tokens
        sc = np.take_along_axis(logits, seen, axis=1)
        sc = np.where(sc < 0, sc * 1.2, sc / 1.2)
        np.put_along_axis(logits, seen, sc, axis=1)
        ids = np.array([[sample(logits[0], rng)]], dtype=np.int64)
        tokens = np.concatenate((tokens, ids), axis=-1)
        if int(ids.flatten()[0]) == STOP:
            break
        mask = np.concatenate([mask, np.ones((b, 1), dtype=np.int64)], axis=1)
        pos = pos[:, -1:] + 1
        for j, k in enumerate(past):
            past[k] = present[j]
    t_lm = time.time() - t1
    speech = tokens[:, 1:-1]
    speech = np.concatenate([prompt_token, speech, np.full((1, 3), SILENCE, dtype=np.int64)], axis=1)
    t2 = time.time()
    wav = dec.run(None, dict(speech_tokens=speech, speaker_embeddings=speaker_embeddings, speaker_features=speaker_features))[0].squeeze(axis=0)
    t_dec = time.time() - t2
    wav = trim_silence(wav)
    sf.write(out, wav, SR)
    secs = len(wav) / SR
    print("%s: %.1f s of speech | load %.1fs, language model %.1fs (%d tokens), decoder %.1fs | real-time factor %.2f" % (os.path.basename(out), secs, t_load, t_lm, tokens.shape[1], t_dec, (t_lm + t_dec) / max(secs, 0.1)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
