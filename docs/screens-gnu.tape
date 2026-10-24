# GNU-dialect stills.  Run from the repo root:
#   nix run nixpkgs#vhs -- docs/screens-gnu.tape
Output docs/img/gnu.png

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
Type "../../target/release/rvs factorial-gnu.asm"
Enter
Sleep 2s
Show

Type ":"
Type "mode gnu"
Sleep 250ms
Enter
Sleep 300ms
Type "c"
Sleep 1s
Type "nnnnnnnnnnnnnnnnnnnnnn"
Sleep 800ms
Type "r"
Sleep 2s
Screenshot docs/img/gnu.png

Type ":"
Type "transpile rvs"
Sleep 250ms
Enter
Sleep 600ms
Type "c"
Sleep 1s
Screenshot docs/img/transpile.png

Sleep 700ms
