# Runtime stills for the README.  Run from the repo root:
#   nix run nixpkgs#vhs -- docs/screens.tape
Output docs/img/overview.png

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 17
Set Width 1760
Set Height 1080
Set Padding 28
Set BorderRadius 8
Set TypingSpeed 20ms
Set Framerate 24
Set Theme {"background": "#1e1e2e", "foreground": "#cdd6f4"}

Hide
Type "cd /home/giovanni/Projects/rvspp/docs/demo"
Enter
Type "../../target/release/rvs factorial.asm"
Enter
Sleep 2s
Show

Type "c"
Sleep 1s

# 22 instructions in: three nested fact() frames on the stack.
Type "nnnnnnnnnnnnnnnnnnnnnn"
Sleep 1s

# Reverse-stepping is bound off the Source pane (p is Vim paste there).
Tab
Sleep 250ms
Type "ppp"
Sleep 600ms

Type ":"
Type "stack"
Sleep 250ms
Enter
Sleep 1.6s
Screenshot docs/img/stack.png
Escape
Sleep 300ms
Sleep 500ms

# Finish the run; every recursion level leaves its frame in memory.
Type "r"
Sleep 2.5s

Tab
Sleep 250ms
Tab
Type "z"
Sleep 1s
Screenshot docs/img/mem.png
Type "z"
Sleep 400ms

Tab
Sleep 250ms
Type "z"
Sleep 1s
Screenshot docs/img/exec.png
Type "z"
Sleep 400ms

Type ":"
Type "stats"
Sleep 250ms
Enter
Sleep 1.8s
Screenshot docs/img/stats.png
Escape
Sleep 300ms
Sleep 600ms

Screenshot docs/img/overview.png

Sleep 700ms
