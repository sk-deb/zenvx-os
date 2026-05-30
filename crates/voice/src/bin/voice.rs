//! `zenvx-voice` — reports local voice readiness (graceful-degradation check).

fn main() {
    let a = zenvx_voice::VoiceAvailability::detect();
    println!("whisper.cpp: {}", if a.whisper { "found" } else { "missing" });
    println!("piper:       {}", if a.piper { "found" } else { "missing" });
    if a.ready() {
        println!("voice: ready (local STT+TTS available)");
    } else {
        println!("voice: degraded — falling back to typed input (install whisper.cpp + piper for local voice)");
    }
}
