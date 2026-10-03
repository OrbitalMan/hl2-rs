miniquad 0.4.11 is vendored from its published Rust crate under MIT OR Apache-2.0. Original licenses are in miniquad/.

Windows input changes:
- Convert absolute RAWINPUT positions into motion deltas using primary/virtual desktop dimensions; reset the history when capture changes.
- Fall back from an unknown message scan code to its Windows virtual key, preserving physical scan-code handling when available.
- Buffer legacy mouse motion until the message queue is drained, and use it while captured only when that frame contained no nonzero raw motion. This supports synthesized messages without double-counting normal raw/legacy pairs.

OpenGL depth changes:
- Apply depth comparison independently of the depth-write mask. A translucent pipeline can now test opaque depth without writing depth; the upstream backend disabled testing whenever writes were disabled.
- Temporarily enable depth writes for depth-buffer clears, then restore the current pipeline's mask. This preserves separate sky/world/viewmodel depth clears after translucent draws.

This is an open-source Rust dependency patch, not decompiled game code. Computer Use validation and remaining fidelity gaps are recorded in docs/validation.md.
