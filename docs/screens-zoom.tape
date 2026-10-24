# Zoomed list/register stills.  Run from the repo root:
#   nix run nixpkgs#vhs -- docs/screens-zoom.tape
Output docs/img/listing.png

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
Sleep 800ms
Type "nnnnnnnnnnnnnnnnnnnnnn"
Sleep 800ms
Type "r"
Sleep 2s

Tab
Sleep 250ms
Type "z"
Sleep 1s
Screenshot docs/img/listing.png
Type "z"
Sleep 400ms

Tab
Sleep 250ms
Type "z"
Sleep 1s
Screenshot docs/img/regs.png
Type "z"
Sleep 400ms

Sleep 700ms
