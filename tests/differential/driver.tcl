# Differential driver: source the reference simulator and run kkbatch.
# Usage: wish driver.tcl <asm> <in> <out> [timeout]
# A patched copy of RVS046.tcl (About dialog removed) is sourced so the run
# is not blocked by the 5-second startup modal.

set src [file join [file dirname [info script]] rvs046_patched.tcl]
source $src
if {$argc < 3} { puts "usage: driver.tcl asm in out \[timeout\]"; exit 1 }
set timeout [expr {[llength $argv] > 3 ? [lindex $argv 3] : 3000}]
set res [kkbatch [lindex $argv 0] [lindex $argv 1] [lindex $argv 2] $timeout]
if {$res ne ""} { puts "DRIVER-ERR: $res" }
exit 0
