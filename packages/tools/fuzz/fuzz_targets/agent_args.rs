// labyrinth-agent's argument parser: any command line (words split at 0 bytes) through `decide` on a made-up system; a panic
// when anything it would do as root is outside src/agent/audit.rs's allow-list. Nothing on the computer is touched.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    labyrinth_tools::agent::audit::fuzz_one(data);
});
