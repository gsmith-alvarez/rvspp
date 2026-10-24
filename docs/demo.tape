# RVS++ animated demo -> docs/img/demo.gif
#   nix run nixpkgs#vhs -- docs/demo.tape
Output docs/img/demo.gif

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 15
Set Width 1440
Set Height 880
Set Padding 24
Set BorderRadius 8
Set TypingSpeed 25ms
Set Framerate 24
Set Theme {"background": "#1e1e2e", "foreground": "#cdd6f4"}

Hide
Type "cd /home/giovanni/Projects/rvspp/docs/demo"
Enter
Type "../../target/release/rvs factorial.asm"
Enter
Sleep 2s
Show

# Compile the buffer: listing, machine code and symbol table fill in.
Type "c"
Sleep 1.2s

# Step into the recursion one instruction at a time.
Type "n"
Sleep 240ms
Type "n"
Sleep 240ms
Type "n"
Sleep 240ms
Type "n"
Sleep 240ms
Type "n"
Sleep 240ms
Type "n"
Sleep 400ms

# Reverse-stepping is bound off the Source pane (p is Vim paste there).
Tab
Sleep 300ms
Type "p"
Sleep 240ms
Type "p"
Sleep 240ms
Type "p"
Sleep 500ms

# Live call-stack visualiser, mid-recursion.
Type ":"
Sleep 150ms
Type "stack"
Sleep 250ms
Enter
Sleep 1.8s
Escape
Sleep 500ms

# Slow the run down so the multi-pane animation is visible, then run to halt.
Type ":"
Sleep 150ms
Type "speed 20"
Sleep 250ms
Enter
Sleep 400ms
Type "r"
Sleep 4.5s

# Hardware counters for the finished run.
Type ":"
Sleep 150ms
Type "stats"
Sleep 250ms
Enter
Sleep 2.2s
Escape
Sleep 1.6s

Sleep 700ms
