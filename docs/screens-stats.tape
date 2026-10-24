# Hardware-counter still. Run from the repo root:
#   nix run nixpkgs#vhs -- docs/screens-stats.tape
Output docs/img/stats.png

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
Type "nnnnnnnnnnnnnnnnnnnnnn"
Sleep 600ms
Type "r"
Sleep 2.5s

Type ":"
Type "stats"
Sleep 300ms
Enter
Sleep 2s
Screenshot docs/img/stats.png
Sleep 900ms
