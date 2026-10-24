#!/bin/sh
# the next line restarts using wish\
exec wish "$0" "$@" 

if {![info exists vTcl(sourcing)]} {

    package require Tk
    switch $tcl_platform(platform) {
	windows {
	    option add *Button.padY 0
	}
	default {
	    option add *Scrollbar.width 10
	    option add *Scrollbar.highlightThickness 0
	    option add *Scrollbar.elementBorderWidth 2
	    option add *Scrollbar.borderWidth 2
	}
    }
    
}

#############################################################################
# Visual Tcl v8.6.0.5 Project
#


#################################
# VTCL LIBRARY PROCEDURES
#

if {![info exists vTcl(sourcing)]} {
#############################################################################
## Library Procedure:  Window

proc ::Window {args} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    global vTcl
    foreach {cmd name newname} [lrange $args 0 2] {}
    set rest    [lrange $args 3 end]
    if {$name == "" || $cmd == ""} { return }
    if {$newname == ""} { set newname $name }
    if {$name == "."} { wm withdraw $name; return }
    set exists [winfo exists $newname]
    switch $cmd {
	show {
	    if {$exists} {
		wm deiconify $newname
	    } elseif {[info procs vTclWindow$name] != ""} {
		eval "vTclWindow$name $newname $rest"
	    }
	    if {[winfo exists $newname] && [wm state $newname] == "normal"} {
		vTcl:FireEvent $newname <<Show>>
	    }
	}
	hide    {
	    if {$exists} {
		wm withdraw $newname
		vTcl:FireEvent $newname <<Hide>>
		return}
	}
	iconify { if $exists {wm iconify $newname; return} }
	destroy { if $exists {destroy $newname; return} }
    }
}
#############################################################################
## Library Procedure:  vTcl:DefineAlias

proc ::vTcl:DefineAlias {target alias widgetProc top_or_alias cmdalias} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    global widget
    set widget($alias) $target
    set widget(rev,$target) $alias
    if {$cmdalias} {
	interp alias {} $alias {} $widgetProc $target
    }
    if {$top_or_alias != ""} {
	set widget($top_or_alias,$alias) $target
	if {$cmdalias} {
	    interp alias {} $top_or_alias.$alias {} $widgetProc $target
	}
    }
}
#############################################################################
## Library Procedure:  vTcl:DoCmdOption

proc ::vTcl:DoCmdOption {target cmd} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    ## menus are considered toplevel windows
    set parent $target
    while {[winfo class $parent] == "Menu"} {
        set parent [winfo parent $parent]
    }

    regsub -all {\%widget} $cmd $target cmd
    regsub -all {\%top} $cmd [winfo toplevel $parent] cmd

    uplevel #0 [list eval $cmd]
}
#############################################################################
## Library Procedure:  vTcl:FireEvent

proc ::vTcl:FireEvent {target event {params {}}} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    ## The window may have disappeared
    if {![winfo exists $target]} return
    ## Process each binding tag, looking for the event
    foreach bindtag [bindtags $target] {
	set tag_events [bind $bindtag]
	set stop_processing 0
	foreach tag_event $tag_events {
	    if {$tag_event == $event} {
		set bind_code [bind $bindtag $tag_event]
		foreach rep "\{%W $target\} $params" {
		    regsub -all [lindex $rep 0] $bind_code [lindex $rep 1] bind_code
		}
		set result [catch {uplevel #0 $bind_code} errortext]
		if {$result == 3} {
		    ## break exception, stop processing
		    set stop_processing 1
		} elseif {$result != 0} {
		    bgerror $errortext
		}
		break
	    }
	}
	if {$stop_processing} {break}
    }
}
#############################################################################
## Library Procedure:  vTcl:Toplevel:WidgetProc

proc ::vTcl:Toplevel:WidgetProc {w args} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    if {[llength $args] == 0} {
        ## If no arguments, returns the path the alias points to
        return $w
    }
    set command [lindex $args 0]
    set args [lrange $args 1 end]
    switch -- [string tolower $command] {
        "setvar" {
            foreach {varname value} $args {}
            if {$value == ""} {
                return [set ::${w}::${varname}]
            } else {
                return [set ::${w}::${varname} $value]
            }
        }
        "hide" - "show" {
            Window [string tolower $command] $w
        }
        "showmodal" {
            ## modal dialog ends when window is destroyed
            Window show $w; raise $w
            grab $w; tkwait window $w; grab release $w
        }
        "startmodal" {
            ## ends when endmodal called
            Window show $w; raise $w
            set ::${w}::_modal 1
            grab $w; tkwait variable ::${w}::_modal; grab release $w
        }
        "endmodal" {
            ## ends modal dialog started with startmodal, argument is var name
            set ::${w}::_modal 0
            Window hide $w
        }
        default {
            uplevel $w $command $args
        }
    }
}
#############################################################################
## Library Procedure:  vTcl:WidgetProc

proc ::vTcl:WidgetProc {w args} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    if {[llength $args] == 0} {
	## If no arguments, returns the path the alias points to
	return $w
    }

    set command [lindex $args 0]
    set args [lrange $args 1 end]
    uplevel $w $command $args
}
#############################################################################
## Library Procedure:  vTcl:toplevel

proc ::vTcl:toplevel {args} {
    ## This procedure may be used free of restrictions.
    ##    Exception added by Christian Gavin on 08/08/02.
    ## Other packages and widget toolkits have different licensing requirements.
    ##    Please read their license agreements for details.

    uplevel #0 eval toplevel $args
    set target [lindex $args 0]
    namespace eval ::$target {set _modal 0}
}
}


if {[info exists vTcl(sourcing)]} {

proc vTcl:project:info {} {
    set base .top44
    namespace eval ::widgets::$base {
        set set,origin 1
        set set,size 1
        set runvisible 1
    }
    set site_3_0 $base.cpd69
    set site_4_0 $site_3_0.cpd56
    set site_3_0 $base.fra76
    set site_4_0 $site_3_0.cpd70
    set site_5_0 $site_4_0.01
    set site_6_0 $site_5_0.cpd65
    set site_7_0 $site_6_0.cpd57
    set site_8_0 $site_7_0.01
    set site_9_0 $site_8_0.cpd57
    set site_10_0 $site_9_0.cpd56
    set site_10_0 $site_9_0.cpd93
    set site_8_0 $site_7_0.02
    set site_9_0 $site_8_0.cpd100
    set site_10_0 $site_9_0.cpd57
    set site_10_0 $site_9_0.cpd102
    set site_5_0 $site_4_0.02
    set site_6_0 $site_5_0.cpd74
    set site_7_0 $site_6_0.01
    set site_8_0 $site_7_0.cpd69
    set site_9_0 $site_8_0.cpd72
    set site_10_0 $site_9_0.01
    set site_11_0 $site_10_0.cpd57
    set site_12_0 $site_11_0.fra77
    set site_12_0 $site_11_0.cpd93
    set site_10_0 $site_9_0.02
    set site_11_0 $site_10_0.cpd100
    set site_12_0 $site_11_0.cpd68
    set site_12_0 $site_11_0.cpd102
    set site_7_0 $site_6_0.02
    set site_8_0 $site_7_0.fra56
    set site_9_0 $site_8_0.cpd57
    set site_10_0 $site_9_0.01
    set site_11_0 $site_10_0.cpd63
    set site_12_0 $site_11_0.cpd68
    set site_12_0 $site_11_0.cpd102
    set site_10_0 $site_9_0.02
    set site_11_0 $site_10_0.cpd62
    set site_12_0 $site_11_0.cpd60
    set site_13_0 $site_12_0.cpd68
    set site_13_0 $site_12_0.cpd102
    set site_12_0 $site_11_0.cpd61
    set site_13_0 $site_12_0.cpd102
    set site_14_0 $site_13_0.fra56
    set site_14_0 $site_13_0.fra57
    namespace eval ::widgets_bindings {
        set tagslist {_TopLevel _vTclBalloon}
    }
    namespace eval ::vTcl::modules::main {
        set procs {
            init
            main
            kkdef
        }
        set compounds {
        }
        set projectType single
    }
}
}

#################################
# USER DEFINED PROCEDURES
#
#############################################################################
## Procedure:  main

proc ::main {argc argv} {
puts "main- after widgets"
kkdef
set ::brk 0
}
#############################################################################
## Procedure:  kkdef

proc ::kkdef {} {
#
TextSource configure -undo 1 ;#to activate CTRL+z (undo), CTRL+y (redo) in the Source window
if {1} { ;#kkgui all in this if
    global widget
	
	#The initial geometry is set here (overrrides the numbers in the Toplevel1 widget!
    wm geometry $widget(Toplevel1) 1234x600+3+3; #update

    set ::kktitle "RVS (RISC-V Visual Simulator) v0.46"
    
    set ::kkabout "***** BETA VERSION FOR EDUCATIONAL USE *****\
\n\nCopyright (c)2019. SIDIC. All Rights Reserved.\
\n\nPermission to use this software for educational not-for-profit purposes without fee is hereby granted, provided that the above copyright notice, this paragraph and the following two paragraphs appear in all copies. Contact sidicinfo@yahoo.co.jp for commercial licensing opportunities.\
\n\nIN NO EVENT SHALL SIDIC BE LIABLE TO ANY PARTY FOR DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL DAMAGES, INCLUDING LOST PROFITS, ARISING OUT OF THE USE OF THIS SOFTWARE AND ITS DOCUMENTATION, EVEN IF SIDIC HAS BEEN ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.\
\n\nSIDIC SPECIFICALLY DISCLAIMS ANY WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. THE SOFTWARE AND ACCOMPANYING DOCUMENTATION, IF ANY, PROVIDED HEREUNDER IS PROVIDED 'AS IS'. SIDIC HAS NO OBLIGATION TO PROVIDE MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR MODIFICATIONS."

    wm title $widget(Toplevel1) $::kktitle

    set ::tfoo 0
    proc AboutEnd {{t 0}} {
        after cancel AboutEnd
        if {$t!=0} {set ::tfoo $t}
        if {$::tfoo<=0} { ;#end of wait
            .foo.button0 configure -text "OK"
            .foo.button0 configure -state normal
        } else { ;#counting 
            .foo.button0 configure -state disabled
            .foo.button0 configure -width 2
            .foo.button0 configure -text "$::tfoo"
            incr ::tfoo -1
            after 1000 AboutEnd
        }
    }
    proc About {{t 0}} { ;#t in seconds
        after 0 [list AboutEnd $t]
after 0 {.foo.msg configure -wraplength 700}
after 0 {wm geometry .foo 800x500-3+3}
        set reply [tk_dialog .foo "" "$::kktitle\n\n$::kkabout" info 0 "   OK   "]
    }
    #About 5; #update
if {0} {
grab release .foo

winfo children .foo
.foo.bot .foo.top .foo.msg .foo.bitmap .foo.button0

.foo.button0 configure -state disabled
.foo.button0 configure -state normal

.foo.button0 configure -width 10
.foo.button0 configure -text "3"

.foo.msg configure -wraplength 500
wm geometry .foo 700x600
}

wm protocol $widget(Toplevel1) WM_DELETE_WINDOW {Exit}
bind $widget(EntryInput) <Return> {EntryInputCmd}

    proc kktxtw {w txt {del 0} {end 0}}  {
        global widget
        $widget($w) configure -state normal
        if {$del} {$widget($w) delete 1.0 {end -1c}}
        $widget($w) insert end $txt
        if {$end} {$widget($w) see end-1line}
        $widget($w) configure -state disabled
    }
    
    proc kkmark {w txt {col red} {line 1}}  {
        global widget
#puts "kkmark: txt=$txt col=$col line=$line"
        $widget($w) configure -state normal
        $widget($w) tag delete col1
        $widget($w) tag configure col1 -foreground $col
        set lst [$widget($w) search  -all $txt 1.0]
        foreach m $lst {
#puts "kkmark: m=$m"
            if {$line==1} {
                $widget($w) tag add col1 "$m linestart" "$m lineend"
            } else {
                $widget($w) tag add col1 "$m" "$m+[string length $txt]c"
            }
        }
#        $widget($w) configure -state disabled
    }

    proc TextRegsCmd {}  {
        global widget
        set res [regp]
        kktxtw TextRegs $res 1 0
#        $widget(TextRegs) delete 1.0 {end -1c}
#        $widget(TextRegs) insert end $res
    }
    proc TextMemoryCmd {}  {
        global widget
        set res [memp]
        kktxtw TextMemory $res 1 0
#        $widget(TextMemory) delete 1.0 {end -1c}
#        $widget(TextMemory) insert end $res
    }
    proc TextOutputCmd {}  {
        global widget
        kktxtw TextOutput "" 1 0
#        $widget(TextOutput) delete 1.0 {end -1c}
    }
    proc TextExecutionCmd {}  {
        global widget
        kktxtw TextExecution "" 1 0
#        $widget(TextExecution) delete 1.0 {end -1c}
    }
    proc CompileCmd {}  {
        global widget
        set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
#        $widget(TextListing) delete 1.0 {end -1c}
        set ::data [$widget(TextSource) get 1.0 {end -1c}]
#  puts $::data
        if {[catch {asm $::data} err]} {
#            $widget(TextListing) insert end "$::res$err\n[et]\n"; $widget(TextListing) see end
            kktxtw TextListing "${::res}ERROR: $err\n[et]\n" 1 1
            set ::sym(START) 0xfffffffffffffffc; #no machine instruction generated (last 4 byte cell in memory)
            set ::PC [a64 $::sym(START)]
        } else {
#            $widget(TextListing) insert end "$::res\n"; $widget(TextListing) see end
            kktxtw TextListing "$::res\n" 1 1
        }
        kkmark TextListing ERROR:
        TextRegsCmd ;#update (zeroed in asm)
        TextMemoryCmd ;#Refresh
        TextExecutionCmd ;#clear
        TextOutputCmd ;#Clear
        if {![info exists ::sym(INPC)] || ![info exists ::sym(INPD)] || ![info exists ::DMAVar]} {
            set ::CheckbuttonINPVar 0; $widget(CheckbuttonINP) configure -state disabled; CheckbuttonINPCmd
        } else {
            $widget(CheckbuttonINP) configure -state normal
        }
        if {![info exists ::sym(OUTC)] || ![info exists ::sym(OUTD)] || ![info exists ::DMAVar]} {
            set ::CheckbuttonOUTVar 0; $widget(CheckbuttonOUT) configure -state disabled; CheckbuttonOUTCmd
        } else {
            $widget(CheckbuttonOUT) configure -state normal
        }
    }
    proc RunCmd {{init 0}}  { ;# init 1 is restart
        global widget
#puts "RunCmd1 init=$init [$widget(ButtonRun) cget -state]"        
#        if {[$widget(ButtonRun) cget -state]=="disabled"} {return}
#puts "RunCmd2 init=$init"        
        if {$init} { ;#restart from the START label address
#puts "RunCmd3 init=$init"        
            ResetPCCmd; 
#            $widget(TextExecution) insert end "START\n"
            kktxtw TextExecution "START\n" 0 1
            set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
        } else {  ;#continue from the address in the PC field
            if {[string is wide $::PC]} {
                set ::PC [a64 $::PC]
            } else {
#                $widget(TextExecution) insert end "ERROR: PC not an address\n"; 
#                $widget(TextExecution) see end-1line
                kktxtw TextExecution "ERROR: PC not an address\n" 0 1
kkmark TextExecution ERROR:
                return
            }
#            $widget(TextExecution) insert end "CONT\n"
            kktxtw TextExecution "RUN\n" 0 1
        }
        
        foreach b {ButtonRun ButtonSTART ButtonStep ButtonCompile} {$widget($b) configure -state "disabled"}
        br 0
        while {$::brk==0} {
            update
            if {[catch {step} err]} {
                set ::PC [a64 $::PC-4] 
#                set err "$err\n"
#                if {[lindex $err 0]!="ebreak"} {set err "ERROR: $err\n"}
                if {[string first "ebreak" $err]!=0} {set err "ERROR: $err\n"}
            } else {
                set err ""
            }
            #puts "res=$::res"
            #puts "err=$err"    
            #$widget(TextExecution) delete 1.0 {end -1c}
      #      $widget(TextExecution) insert end $::res
#            $widget(TextExecution) insert end $err
#            $widget(TextExecution) see end-1line
            if {$::brk!=0} {append err "ERROR: Stopped"}
            kktxtw TextExecution $err 0 1
            TextRegsCmd
            TextMemoryCmd
            if {$err!=""} {break}
        }
        StopCmd
kkmark TextExecution ERROR:
    }
    proc StepCmd {}  {
        global widget
        
        ;#continue from the address in the PC field
        if {[string is wide $::PC]} {
            set ::PC [a64 $::PC]
        } else {
#            $widget(TextExecution) insert end "ERROR: PC not an address\n"; 
#            $widget(TextExecution) see end-1line
            kktxtw TextExecution "ERROR: PC not an address\n" 0 1
kkmark TextExecution ERROR:
            return
        }
#        $widget(TextExecution) insert end "STEP\n"
        kktxtw TextExecution "NEXT\n" 0 1
        
        if {[catch {step} err]} {
            set ::PC [a64 $::PC-4]
#            set err "$err\n"
            if {[lindex $err 0]!="ebreak"} {set err "ERROR: $err\n"}
        } else {   
            set err ""
        }
        #puts "res=$::res"
        #puts "err=$err"    
        #$widget(TextExecution) delete 1.0 {end -1c}
     #   $widget(TextExecution) insert end $::res
#        $widget(TextExecution) insert end $err
#        $widget(TextExecution) see end-1line
        kktxtw TextExecution "$err" 0 1
kkmark TextExecution ERROR:
        TextRegsCmd
        TextMemoryCmd
    }
    proc step {{pc -1}} {
        global widget
        if {$pc!=-1} {set ::PC [a64 $pc]}
#puts "[a64 $::PC]"
        set ::PC [a64 $::PC+4]; #PC increased before the execution
        #set ::res ""; set ::res "[a64 $::PC-4] $::mem([a64 $::PC-4])\n"
        set ::res ""; 
        if {![info exists ::mem([a64 $::PC-4])]} {error "[a64 $::PC-4]: NO INSTRUCTION"}
        set ::res "[a64 $::PC-4] [fmti "[lindex $::mem([a64 $::PC-4]) 0] [lindex $::mem([a64 $::PC-4]) 1]"]"
        if {$::CheckbuttonTXTExeVar} {
            set ::res "$::res [lns [lindex $::mem([a64 $::PC-4]) 2] 1]"
        }
        set ::res "$::res\n"
#        $widget(TextExecution) insert end $::res
#        $widget(TextExecution) see end-1line
        kktxtw TextExecution "$::res" 0 1
        eval [string toupper [lindex $::mem([a64 $::PC-4]) 0]] [lindex $::mem([a64 $::PC-4]) 1]
    }

    proc CancelCmd {} {
        set ::ENTER 2
    }
    proc StopCmd {} {
        global widget
        CancelCmd
        br; after 3000 {br 0}
        foreach b {ButtonRun ButtonSTART ButtonStep ButtonCompile} {$widget($b) configure -state "normal"}
    }
    proc ResetPCCmd {} {catch {set ::PC [a64 $::sym(START)]} err}
    
    proc CheckbuttonHEXCompCmd {} {TextMemoryCmd}; set ::CheckbuttonHEXCompVar 1
    proc CheckbuttonBINCmd     {} {TextMemoryCmd}; set ::CheckbuttonBINVar 1
    proc CheckbuttonINTCompCmd {} {TextMemoryCmd}; set ::CheckbuttonINTCompVar 1
    proc CheckbuttonTXTCompCmd {} {TextMemoryCmd}; set ::CheckbuttonTXTCompVar 1

    proc CheckbuttonTXTExeCmd {} {TextMemoryCmd}; set ::CheckbuttonTXTExeVar 1

    proc CheckbuttonINTRegCmd {} {TextRegsCmd}; set ::CheckbuttonINTRegVar 1
    proc CheckbuttonFLPRegCmd {} {TextRegsCmd}; set ::CheckbuttonFLPRegVar 1
    proc CheckbuttonHEXRegCmd {} {TextRegsCmd}; set ::CheckbuttonHEXRegVar 1
    
    proc CheckbuttonHEXCmd {} {TextMemoryCmd}; set ::CheckbuttonHEXVar 1
    proc CheckbuttonINTCmd {} {TextMemoryCmd}; set ::CheckbuttonINTVar 1
    proc CheckbuttonFLPCmd {} {TextMemoryCmd}; set ::CheckbuttonFLPVar 1
    proc CheckbuttonTXTMemCmd {} {TextMemoryCmd}; set ::CheckbuttonTXTMemVar 1
    
    proc CheckbuttonOUTCmd {} {after cancel Output; if {$::CheckbuttonOUTVar} {Output}}; set ::CheckbuttonOUTVar 0
    proc Output {} {
        global widget
        #puts "Output"
        if {[info exists ::mem([a64 0])] && "[lindex $::mem([a64 0]) 0]"=="DD"} { ;#OUTC output device control register
        #puts "Output1"
            set m0 $::mem([a64 0]) ;#control reg
            if {[lindex $m0 1]!="0x0000000000000000"} { ;#output requested
                set m01 [x2s [lindex $m0 1]] ;#control "I"nteger,"F"loat, or "C"hars
                set m11 [lindex $::mem([a64 8]) 1] ;#data reg
                if {[string first "I" $m01]>=0} { ;#Integer
                    set res [x2i $m11]
                } elseif {[string first "F" $m01]>=0} { ;#Float
                    set res [x2f $m11]
                } elseif {[string first "C" $m01]>=0} { ;#Chars
                    set res [x2s [revx $m11]]
                } elseif {[string first "S" $m01]>=0} { ;#Hex
#                    set res "CONTROL YET NOT IMPLEMENTED:$m01"
                    set res [getstr1 $m11]
                } elseif {[string first "X" $m01]>=0} { ;#Hex
                    set res $m11
                } else {
                    set res "WRONG CONTROL:$m01"
                }
#                $widget(TextOutput) insert end "$res\n"
#                $widget(TextOutput) see end-1line
                kktxtw TextOutput "$res\n" 0 1
                set ::mem([a64 0]) [lreplace $m0 1 1 [a64 0]]
            }
        }
        after 200 Output
    }
    proc CheckbuttonINPCmd {} {
        global widget
        after cancel Input; 
        set ::EntryLabelType ""
        if {$::CheckbuttonINPVar} {
            Input
        } else {
            $widget(EntryLabel) configure -fg black
            $widget(EntryLabel) configure -bg gray75
        }
    }; set ::CheckbuttonINPVar 0
    proc blink {{bl 1}} {
        global widget
        if {$bl==0} {
            after cancel blink
            $widget(EntryLabel) configure -bg gray75
        } else {
            if {[$widget(EntryLabel) cget -bg]=="gray75"} {
                $widget(EntryLabel) configure -bg red
            } else {
                $widget(EntryLabel) configure -bg gray75
            }
            after 500 {blink}
        }
    }
    proc EntryInputCmd {}  { ;#Enter pressed see ECALL, too!
        global widget
#        after cancel Input       
#        puts "EntryInputVar=$::EntryInputVar"
    if {$::EntryInputVar=="console"} {set ::EntryInputVar ""; console show}
    if {$::EntryInputVar=="batch"} {set ::BatchVar ""}
    if {$::EntryInputVar=="dma"} {set ::DMAVar ""}
        if {$::CheckbuttonINPVar && [info exists ::mem([a64 0x10])] && "[lindex $::mem([a64 0x10]) 0]"=="DD"} { ;#INPC input device control register
            set m0 $::mem([a64 0x10]) ;#control reg
            set m1 $::mem([a64 0x18]) ;#data reg
            if {[lindex $m0 1]!="0x0000000000000000"} { ;#input requested
                set m01 [x2s [lindex $m0 1]] ;#control "I"nteger,"F"loat, or "C"hars
                set m11 [lindex $::mem([a64 0x18]) 1] ;#data reg
                if {[string first "I" $m01]>=0} { ;#Integer
                    if {[catch {set res [i2x $::EntryInputVar]} err]} {
#                        $widget(TextOutput) insert end "$::EntryInputVar<-ERROR: NOT AN INTEGER\n"; $widget(TextOutput) see end-1line
                        kktxtw TextOutput "$::EntryInputVar<-ERROR: NOT AN INTEGER\n" 0 1
                        set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
                        return
                    } else {
                        set res [i2x $::EntryInputVar]
                    }
                } elseif {[string first "F" $m01]>=0} { ;#Float
                    if {[catch {set res [f2x $::EntryInputVar]} err]} {
#                        $widget(TextOutput) insert end "$::EntryInputVar<-ERROR: NOT A FLOAT\n"; $widget(TextOutput) see end-1line
                        kktxtw TextOutput "$::EntryInputVar<-ERROR: NOT A FLOAT\n" 0 1
                        set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
                        return
                    } else {
                        set res [f2x $::EntryInputVar]
                    }
                } elseif {[string first "C" $m01]>=0} { ;#Chars
                    set res [revx [s2x $::EntryInputVar]]
                } elseif {[string first "S" $m01]>=0} { ;#String
                    #set res [putstr $m11 $::EntryInputVar] ;#String; the staring address is in INPD
                } else {
                    set res [s2x "ERR:$m01"]
                }
                if {$::CheckbuttonINPVar} {
                    if {[string first "S" $m01]>=0} { ;#String
#        puts "EntryInputCmd: m11=$m11" 
                        set res [putstr $m11 $::EntryInputVar\0] ;#String; the staring address is in INPD
                        if {$res!=""} {
#                            $widget(TextOutput) insert end "$::EntryInputVar\n$res\n"; $widget(TextOutput) see end-1line
                            kktxtw TextOutput "$::EntryInputVar\n$res\n" 0 1
                            set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
                            return
                        }
                    } else {
                        set ::mem([a64 0x18]) [lreplace $m1 1 1 $res] ;#set data reg
                    }
                    set ::mem([a64 0x10]) [lreplace $m0 1 1 [a64 0]] ;#set control reg to 0
                    set ::INPprompt 0 ;#input prompt not shown yet (will be the first call of Input)
#                    $widget(TextOutput) insert end "$::EntryInputVar\n"; $widget(TextOutput) see end-1line; 
                    kktxtw TextOutput "$::EntryInputVar\n" 0 1
                    set ::EntryInputVar ""
                    set ::EntryLabelType ""
                }
            }
        } else { ;#no INP device
        }
     set ::ENTER 1
        TextRegsCmd
        TextMemoryCmd
    }; set ::INPprompt 1
    proc Input {} { ;#repeatedly called with after
        global widget
        #puts "Input"
        if {[info exists ::mem([a64 0x10])] && "[lindex $::mem([a64 0x10]) 0]"=="DD"} { ;#INPC output device control register
            set m0 $::mem([a64 0x10]) ;#control reg
            if {[lindex $m0 1]!="0x0000000000000000"} {
                set m01 [x2s [lindex $m0 1]] ;#control "I"nteger,"F"loat, or "C"hars
                if {[string first "I" $m01]>=0} { ;#Integer
                    set ::EntryLabelType "I"
                } elseif {[string first "F" $m01]>=0} { ;#Float
                    set ::EntryLabelType "F"
                } elseif {[string first "C" $m01]>=0} { ;#Chars
                    set ::EntryLabelType "C"
                } elseif {[string first "S" $m01]>=0} { ;#Hex
                    set ::EntryLabelType "S"
                } else {
                    set res "ERR:$m01"
                }
                if {[$widget(EntryLabel) cget -bg]=="gray75"} {
                    #$widget(EntryLabel) configure -fg red
                    $widget(EntryLabel) configure -bg red
                } else {
                    #$widget(EntryLabel) configure -fg black
                    $widget(EntryLabel) configure -bg gray75
                }
                if {!$::INPprompt} { ;#input prompt not shown yet (first call of Input)
#                    $widget(TextOutput) insert end "\n$::EntryLabelType?"; $widget(TextOutput) see end-1line
                    kktxtw TextOutput "\n$::EntryLabelType?" 0 1
                    focus $widget(EntryInput)
                    set ::INPprompt 1 ;#input prompt was shown already
                }
            } else {
                #$widget(EntryLabel) configure -fg black
                $widget(EntryLabel) configure -bg gray75
            }
        }
        after 500 Input
    }
    
#   https://wiki.tcl-lang.org/1461
    proc toplist {{W .}} { ;#show all top windows
        set list {}
        if { [string equal [winfo toplevel $W] $W] } {
            lappend list $W
        }
        foreach w [winfo children $W] {
            set list [concat $list [toplist $w]]
        }
        return $list
     } 
 
    set ::Fname ""
    set ::Ftypes {  {{Assembly} {.a .s .asm}}  {{Text Files} {.txt}}  {{All Files} *}  }
    set ::Fchange 0 ;#changes after load
    proc Load {} {
        global widget
        if {[$widget(TextSource) edit modified]} {
#tk busy $widget(Toplevel1); update
#wm attributes $widget(Toplevel1) -disabled 1
            # - parent is IMPORTANT. It makes it modal dialogue 
            set answer [tk_messageBox -parent $widget(Toplevel1) -icon question -type yesnocancel  \
                        -message "Save Changes?" -detail "You have unsaved changes. Do you wish to Save?"]
#tk busy forget $widget(Toplevel1)
#wm attributes $widget(Toplevel1) -disabled 0
            switch -- $answer {
                yes Save
                cancel return
            }
        }        
        set filename [tk_getOpenFile -parent $widget(Toplevel1) -filetypes $::Ftypes -title "Load"]
        if {$filename ne ""} {
            # Open the file ...
            set ::Fname $filename
            #  Slurp up the data file
            set fp [open "$::Fname" r]; set file_data [read $fp]; close $fp
            $widget(TextSource) delete 1.0 {end -1c}
            $widget(TextSource) insert end $file_data
            $widget(TextSource) see end
            $widget(TextSource) edit modified false    
        }
    }
    proc Append {} {
        global widget
        if {[$widget(TextSource) edit modified]} {
            set answer [tk_messageBox -parent $widget(Toplevel1) -icon question -type yesnocancel  \
                        -message "Save Changes?" -detail "You have unsaved changes. Do you wish to Save?"]
            switch -- $answer {
                yes Save
                cancel return
            }
        }        
        set filename [tk_getOpenFile -parent $widget(Toplevel1) -filetypes $::Ftypes -title "Append"]
        if {$filename ne ""} {
            # Open the file ...
            set ::Fname $filename
            #  Slurp up the data file
            set fp [open "$::Fname" r]; set file_data [read $fp]; close $fp
#            $widget(TextSource) delete 1.0 {end -1c}
            $widget(TextSource) insert end $file_data
            $widget(TextSource) see end
            $widget(TextSource) edit modified true    
            set ::Fname ""
        }
    }
    proc SaveAs {} {
        global widget
            #set filename [tk_getSaveFile -filetypes $types -initialfile $::Fname -defaultextension .asm]
            set filename [tk_getSaveFile -parent $widget(Toplevel1) -filetypes $::Ftypes -initialfile [file tail $::Fname] -title "SaveAs"]
            if {$filename ne ""} {
                # Save the file ...
                set ::Fname $filename
                set file_data [$widget(TextSource) get 1.0 {end -1c}]
                set fp [open "$::Fname" w]; puts -nonewline $fp $file_data; close $fp
                $widget(TextSource) edit modified false    
            }
    }
    proc Save {} {
        global widget
            if {$::Fname==""} {
                SaveAs
            } else {
                # Save the file ...
                set file_data [$widget(TextSource) get 1.0 {end -1c}]
                set fp [open "$::Fname" w]; puts -nonewline $fp $file_data; close $fp
                $widget(TextSource) edit modified false    
            }
    }
    proc Exit {} {
        global widget
        if {[$widget(TextSource) edit modified]} {
            set answer [tk_messageBox -parent $widget(Toplevel1) -icon question -type yesnocancel \
                        -message "Save Changes?" -detail "You have unsaved changes. Do you wish to Save?"]
            switch -- $answer {
                yes Save
                cancel return
            }
        }        
        exit
    }
    
    set ::FtypesB {  {{Batch} {.b}}  {{Text Files} {.txt}}  {{All Files} *}  }
    proc Batch {} {
		if {![info exists ::BatchVar]} {return}
        global widget
        if {[$widget(TextSource) edit modified]} {
#tk busy $widget(Toplevel1); update
#wm attributes $widget(Toplevel1) -disabled 1
            # - parent is IMPORTANT. It makes it modal dialogue 
            set answer [tk_messageBox -parent $widget(Toplevel1) -icon question -type yesnocancel  \
                        -message "Save Changes?" -detail "You have unsaved changes. Do you wish to Save?"]
#tk busy forget $widget(Toplevel1)
#wm attributes $widget(Toplevel1) -disabled 0
            switch -- $answer {
                yes Save
                cancel return
            }
        }        
        set filename [tk_getOpenFile -parent $widget(Toplevel1) -filetypes $::FtypesB -title "Batch"]
        if {$filename ne ""} {
            set err ""
            set fp [open "$filename" r]; set file_data [read $fp]; close $fp
            set lines [split $file_data \n]
            foreach line $lines {
#puts "line=$line"
                set line [string trim $line]
                append err "$line\n"
                if {$line!="" && [string index $line 0]!="#"} { 
                    set res "[kkbatch [lindex $line 0] [lindex $line 1] [lindex $line 2] [lindex $line 3]]"
                    if {$res!=""} {
                        append err "#$res\n"
                    }
                }
            }
#            set fp [open $filename.err w]; puts -nonewline $fp $err; close $fp
            set ::Fname $filename
            $widget(TextSource) delete 1.0 {end -1c}
            CompileCmd
            $widget(TextSource) insert end $err
            $widget(TextSource) see end
            $widget(TextSource) edit modified false   
            append data [$widget(TextSource) get 1.0 {end -1c}]
            kkmark TextSource ERROR:
        }
    }
    proc kkbatch {asm in out {timeout 3000}} {
        global widget
        set err ""
        if {$timeout==""} {set timeout 3000}
        set stars "\n*********************************************************\n"
        set data "${stars}Load $asm${stars}"        
        # Open the file ...
        set ::Fname $asm
        #  Slurp up the data file
#        set fp [open "$::Fname" r]; set file_data [read $fp]; close $fp
        if {[catch {set fp [open "$::Fname" r]; set file_data [read $fp]; close $fp} err]} {
            return "ERROR: $err"
        }
        $widget(TextSource) delete 1.0 {end -1c}
        $widget(TextSource) insert end $file_data
        $widget(TextSource) see end
        $widget(TextSource) edit modified false   
        append data [$widget(TextSource) get 1.0 {end -1c}]
        
        #read the input data from file
#        set fp [open "$in" r]; set ind [read $fp]; close $fp
        if {[catch {set fp [open "$in" r]; set ind [read $fp]; close $fp} err]} {
            return "ERROR: $err"
        }
        
        set ::inl [split $ind "\n"]
#puts "::inl=$::inl"
                    
        append data "${stars}Compile${stars}"        
        CompileCmd
        append data [$widget(TextListing) get 1.0 {end -1c}]
        append data "${stars}"        
        append data [$widget(TextMemory) get 1.0 {end -1c}]

        append data "${stars}Run${stars}"        
        after $timeout {br 1}
        RunCmd 1
        after cancel {br 1}
        append data [$widget(TextExecution) get 1.0 {end -1c}]
        append data "${stars}"        
        append data [$widget(TextMemory) get 1.0 {end -1c}]
        append data "${stars}"        
        append data [$widget(TextRegs) get 1.0 {end -1c}]
        append data "${stars}"        
        append data [$widget(TextOutput) get 1.0 {end -1c}]

        unset ::inl

#        set fp [open $out w]; puts -nonewline $fp $data; close $fp
        if {[catch {set fp [open $out w]; puts -nonewline $fp $data; close $fp} err]} {
            return "ERROR: $err"
        }
        #return "OK"
        return ""
    }
#kkbatch batch2.asm batch2.in.txt batch2.out.txt
#kkbatch batch1.asm batch1.in.txt batch1.out.txt 3000
    
#   https://wiki.tcl.tk/881

    proc TextSourceChanged {} {
        global widget
        if {[$widget(TextSource) edit modified]} {
            $widget(LabelFname) configure -fg red
            return 1
        } else {
            $widget(LabelFname) configure -fg black
            return 0
        }
    }
    bind $::widget(TextSource) <<Modified>> { TextSourceChanged }    
#$widget(TextSource) edit modified false    
    
} ;#kkgui end
#
if {1} { ;#kkdef all in this if
    global widget
    
    proc slbl {s} { ;#substitute all the labels with their values, and call expr
        while {[set i [string first "'" $s]]>=0} { ;#single char literal e.g. 'a' -> 0x40
            if {[string index $s $i+2]!="'"} {return "err"}
            binary scan [string index $s $i+1] H* hex
            set s "[string range $s 0 $i-1]0x$hex[string range $s $i+3 end]"
        }
        set s1 " [string map { "(" " ( " ")" " ) " "+" " + " "-" " - " "*" " * " "/" " / " "%" " % " "<<" " << " ">>" " >> "  "&" " & "  "|" " || " } $s] "
    #puts "slbl1: s1=$s1"    
        foreach lbl [array names ::sym] {
            while {1} {
                set s2 [string map [list " $lbl " " $::sym($lbl) "] $s1 ]
    #puts "slbl2: s1=$s1"  
                if {$s2==$s1} {break}
                set s1 $s2
            }
        }
        if {[catch {set s1 [expr $s1]} err]} {
    #puts "slbl3: s1=$s1\nerr=$err"
            set s1 "err"
        }
        return $s1
    } 
    if {0} {
        slbl "b+a a a"
        slbl "(b+a a a)"
        slbl "(b+a a a)"
        expr [slbl "b+a* a -a%8"]
        expr [slbl "b+(a* a) -a%8"]
    }
    
    proc et {} {puts $::errorInfo} ;#error trace to console. Call it to show the error stack
#    proc et {} {return $::errorInfo} ;#error trace to widget. Call it to show the error stack
    set ::brk 0; proc br {{b 1}} { set ::brk $b} ;#br to break; br 0 to remove break 
    proc d {} { parray ::mem; parray ::sym}
    
    proc a64 {s} {format "0x%016lx" [expr $s]}
    proc i2x {i} { ;#integer_string (signed or unsigned) to hexadecimal_string
        set ret [format 0x%016lx $i]
    }
    proc x2i {x} { ;#hexadecimal_string to integer_string (unsigned!)
        set ret [expr $x]
    }
    proc x12i {x} { ;#12-bit hexadecimal_string [0x000-0xfff] to signed integer_string [-2048,2047]
        if {$x> 2047} {set x [expr $x-4096]} ;#12bit unsigned to signed
        set ret [expr $x]
    }
    proc i12x {x} { ;#signed integer_string [-2048,2047] to 12-bit hexadecimal_string [0x00000-0xfffff]
        if {$x<0} {set imm [expr 4096+$x]} ;#12bit signed to unsigned
        return 0x[string range [format %03x $x] end-2 end]
    }
    proc x20i {x} { ;#20-bit hexadecimal_string [0x00000-0xfffff] to signed integer_string [-2048,2047]
        set pw 19
        if {$x>int(pow(2,$pw))-1} {set x [expr $x-int(pow(2,$pw+1))]} ;#20bit unsigned to signed
        set ret [expr $x]
    }
    proc i20x {x} { ;#signed integer_string [-2048,2047] to 20-bit hexadecimal_string [0x000-0xfff]
        set pw 19
        if {$x<0} {set imm [expr int(pow(2,$pw+1))+$x]} ;#20bit signed to unsigned
        return 0x[string range [format %05x $x] end-4 end]
    }
    proc x64i {x} { ;#64-bit hexadecimal_string [0x0000000000000000-0xffffffffffffffff] to signed integer_string [-9223372036854775808,9223372036854775807]
#        set pw 63
#        if {$x>int(pow(2,$pw))-1} {set x [expr $x-int(pow(2,$pw+1))]} ;#64bit unsigned to signed
#        set ret [expr $x]
		set ret "[format "%-20ld" $x]"
    }            
    proc revx {x} { ;#reverse a hex string
        set len [string length $x]; set res ""
        for {set i 0} {$i<$len} {incr i 2} {
            set res "[string range $x $i $i+1]$res"
        }
        if {[string range $res end-1 end]=="0x"} {set res 0x[string range $res 0 end-2]}
        return $res
    }
    proc s2x {c} { ;#character_string to hexadecimal_string from hi to low paddedd with 00s to 8 bytes
        binary scan $c H* hex; set hex "${hex}0000000000000000"
        set ret "0x[string range $hex 0 15]"
        return $ret
    }
    proc x2s {x} { ;#hexadecimal_string to character_string
        if {[string range $x 0 1]=="0x"} {set x [string range $x 2 end]}
        set ret [binary format H* $x]
        return $ret
    }
    
#    https://wiki.tcl.tk/756
    proc f2x {f} { ;#double_string to hexadecimal_string
        set dw [binary format Q $f] ;#double little endian (64bit binary representation of double)
    #    binary scan $dw d ret
        binary scan "$dw" H* ret ;#64bits converted to hex
    #puts "f2x: ret=$ret"    
        return 0x$ret
    }    
    proc x2f {x} { ;#hexadecimal_string to double_string (no errors but returns empty string)
        if {[string range $x 0 1]=="0x"} {set x [string range $x 2 end]}
#        set x [revx $x]
        set dw [binary format H* $x] ;#little endian
        set ret ""; binary scan $dw Q ret
#		if {[string first "NaN" $ret]>=0} {set ret "0"} ;#error
        return $ret
    }
if {0} {
    f2x 2 ;#0000000000000040
    x2f [f2x 2] ;#2.0
    x2f 12345 ;#""
    string length [x2f 12345] ;#0    
}



 
    proc memw {adr val} { ;#write memory at adr, val is a hex string without 0x in-front
#  puts "memw: adr=$adr val=$val"
    set len [expr [string length $val]/2]
        if {$len!=1 && $len!=2 && $len!=4 && $len!=8} {error "memw: #len problem adr=$adr len=$len"; return}
#        if {int($adr)/int($len)*int($len)!=int($adr)} {error "memw: #alignment problem adr=$adr len=$len"; return}
        if {$adr/$len*$len!=$adr} {error "memw: #alignment problem adr=$adr len=$len"; return}
#        set a8 [expr int($adr)/8*8]; set r8 [expr int($adr)%8]; set a8 [a64 $a8]
        set a8 [expr $adr/8*8]; set r8 [expr $adr%8]; set a8 [a64 $a8]
        if {[info exists ::mem([a64 $a8+4])]} {error "memw: #code adr=$adr len=$len mem($a8)=$::mem($a8)"; return}
        if {![info exists ::mem($a8)]} {
            set ::mem($a8) {DD 0x0000000000000000 {}}
        }
        if {"[lindex $::mem($a8) 0]"!="DD"} {error "memw: #not data adr=$adr len=$len mem($a8)=$::mem($a8)"; return}
        set m [lindex $::mem($a8) 1]
#        set ret [string range $m [expr 18-($r8+$len)*2] [expr 17-$r8*2] ]
        set bg [expr 18-($r8+$len)*2]; set en [expr 17-$r8*2]
        set s "[string range $m 0 [expr $bg-1]]$val"
#     puts "memw: s=$s"
        if {$en<17} {set s "$s[string range $m [expr $en+1] end]"}
#     puts "memw: s=$s"
        set ret [lreplace $::mem($a8) 1 1 $s]
#     puts "memw: ret=$ret"
        if {$s!="0x0000000000000000"} {
            set ::mem($a8) $ret
#     puts "memw: set ::mem($a8) $ret"
        } else {
#            unset ::mem($a8) ;#no memory release for now
            set ::mem($a8) $ret
#     puts "memw: unset ::mem($a8)"
        }
#     puts "memw: ret=$ret"
    }
    
    if {0} {
        set a8 1; set a8 [a64 $a8]; set ::mem($a8) {DD "0x................"}
        memw 0 "01"
        memw 1 "45"
        memw 18 "89"
        memw 22 "cd"
        memw 21 "cd"
    }    
    proc memr {adr len} { ;#read memory at adr, len is 1,2,4,8 bytes
        if {$len!=1 && $len!=2 && $len!=4 && $len!=8} {error "memr: #len problem adr=$adr len=$len"; return}
#        if {int($adr)/int($len)*int($len)!=$adr} {error "memr: #alignment problem adr=$adr len=$len"; return}
        if {$adr/$len*$len!=$adr} {error "memr: #alignment problem adr=$adr len=$len"; return}
#        set a8 [expr int($adr)/8*8]; set r8 [expr int($adr)%8]; set a8 [a64 $a8]
        set a8 [expr $adr/8*8]; set r8 [expr $adr%8]; set a8 [a64 $a8]
        if {[info exists ::mem([a64 $a8+4])]} {error "memr: #code adr=$adr len=$len mem($a8)=$::mem($a8)"; return}
        if {![info exists ::mem($a8)]} {
            set m "0x0000000000000000"
        } else {
            if {"[lindex $::mem($a8) 0]"!="DD"} {error "memr: #not data adr=$adr len=$len mem($a8)=$::mem($a8)"; return}
            set m [lindex $::mem($a8) 1]
        }
#        set ret [string range $m [expr 3+(7-$r8)*2+$len*2+1] [expr 3+(7-$r8)*2] ]
        set ret [string range $m [expr 18-($r8+$len)*2] [expr 17-$r8*2] ]
#puts "adr=$adr len=$len a8=$a8 r8=$r8 m=$m ret=$ret"
        return $ret
    }
    
    
    if {0} {
    memr 10 2
    memr 14 4
    }
    proc args2hex {args fmt} { ;# many args will be packed into (multiple) double word hex
#      puts "arg2hex 1 args=$args"
#        set args [string trim $args "{}"]
#      puts "arg2hex 1 args=$args"
        set hex ""; foreach arg $args {binary scan [binary format $fmt $arg] H* s; set hex "$s$hex"} ;#I1
#      puts "arg2hex 2 hex=$hex"
        set len [string length $hex]; set a8 [expr int($len)/16]; set r8 [expr int($len)%16]
        if {$r8>0} {set hex "[string range {0000000000000000} end-15 end]$hex"}
        set args 0x[string range $hex [expr [string length $hex]-16] [expr [string length $hex]-1]]
        for {set i [expr [string length $hex]-32]} {$i>=0} {incr i -16} {set args "$args,0x[string range $hex $i [expr $i+15]]"}
#        puts "arg2hex 3 args=$args"
        return $args
    }
    
    proc lns {ln {f 0}} { ;#linesplit into {label op args comment}
        set ln [string trim $ln]
#  puts "lns: ln=$ln|"
        set cm ""
        if {[set i [string first "#"  $ln]]>-1 || [set i [string first ";"  $ln]]>-1 || [set i [string first "//"  $ln]]>-1} { ;#comment
			set q1 0
			for {set i 0} {$i < [string length $ln]} {incr i} {
				set ch [string index $ln $i]
				if {$ch=="\""} {set q1 [expr 1-$q1]}
				if {$ch=="#" || $ch==";" || ($ch=="/" && [string index $ln $i+1]=="/")} {
					if {$q1==0} {
            set cm [string range $ln $i end]; set ln [string range $ln 0 $i-1]
					}
				}
			}
        }
        set lb ""
        if {([set i [string first ":" $ln]]>-1)} { ;#label
            set lb [string range $ln 0 [expr $i-1]]; set ln [string trim [string range $ln [expr $i+1] end]] ;#-1 exclude the :
        }
        set op [lindex $ln 0]; 
        set args1 [string trim [string range $ln [string length $op] end]]
        set args [lrange $ln 1 end];
#        set args [string trim $args "{}"]; 
   #     set args [string trim $args]; 
        #set args [string map {" " ""} $args]
 #  puts "lns: lb=|$lb| op=|$op| args=|$args| args1=$args1| ln=|$ln| cm=|$cm|"
        if {!$f} {
			set args2 [string map {"\t" " "} $args1]
            set res [list $lb $op $args2 $cm] ;#kk this removed the double quotes; CHANGED $args->$args2
        } else {
            if {$lb!=""} {set lb "$lb:"} ;#add the :
#            if {$args!=""} {set args [list $args]; set args [string map {"{" \" "}" \"} $args]}
            set res [format "%-5s %-6s %-20s %s" $lb $op $args1 $cm]
        }
#puts "lns:res=$res|"
        return $res
    }
if {0} {
    lns {   add x1,x2,x3 ;comment comment}
    lns {a: add x1,x2, x3 ;comment comment}
    lns {b:   DC "test test" ;comment comment}
    lns {b:   DC "test test" ;comment comment} 1
}
    
    proc argsnrm {args} { ;# $x1,imm($x0) -> $x1 $x0 imm 
#puts "argsnrm1: args=$args|"
        set args [string trim $args "{}"]
#puts "argsnrm2: args=$args|"
        if {[string first "(" $args]==-1} { ;# no parentheses
            set args [string map {"," " "} $args] ;# normalize commas
#puts "argsnrm3: args=$args|"
            set args "[lindex $args 0] [lindex $args 1] [lindex $args 2]"
        } else {
            set args [string map {"," " " "(" " " ")" " "} $args] ;# normalize commas and parentheses
#puts "argsnrm4: args=$args|"
            set args "[lindex $args 0] [lindex $args 2] [lindex $args 1]"
        }
        return $args
    }
if {0} {
    argsnrm x1,x2,0
    argsnrm x1, x2,0
    argsnrm x1,0(x2)
    argsnrm x1, 0(x2)
    argsnrm x1,0( x2)
    argsnrm  x0 dd1(x1)
    argsnrm  "x0 dd1(x1)" ;#happens often!
}    

    proc argsnrm {args {op ld}} { ;# $x1,imm($x0) -> $x1 $x0 imm 
#puts "argsnrm1: args=$args|"
        set args [string trim $args "{} "]
#puts "argsnrm2: args=$args|"
        set args [split $args ","] 
        set arg0 [string trim [lindex $args 0] "{} "]
        set arg1 [string trim [lindex $args 1] "{} "]
        set arg2 [string trim [lindex $args 2] "{} "]
        set args [list $arg0 $arg1 $arg2] ;#3 args
#puts "argsnrm3: args=$args|"
        if {$arg2==""} { ;# 2 , args
            set args [list $arg0 $arg1] ;# 2 , args
            if {$op!="auipc" && $op!="jal" && $op!="lui"} { ;#3 args
                set p1 [string last "(" $arg1]
                if {$p1>=0} {
                    set arg11 [string range $arg1 0 $p1-1]
                    set arg12 [string range $arg1 $p1 end]
                    set arg2  [string map {"(" " " ")" " "} $arg12] ;#register
                    set arg2 [string trim $arg2 "{} "]
                    set args [list $arg0 $arg2 $arg11] ;# 3 args
                }
            }
        }
#puts "argsnrm4: args=$args|"
        return $args
    }
if {0} {
    argsnrm "x1,x2,0"
    argsnrm "x1, x2,0"
    argsnrm "x1,0(x2)"
    argsnrm "x1, 0(x2)"
    argsnrm "x1,0( x2)"
    argsnrm "x0 dd1(x1)"
    argsnrm "x0 dd1(x1)" ;#happens often!
    argsnrm "x0,aa+2+3<<2(x1)" 
    argsnrm "x0,aa +2+3<<2(x1)" 
    argsnrm "x0,x1,aa+2+3<<2"
    argsnrm "x0,x1,aa+2+(3<<2)"
    argsnrm "x0,3<<2" ;#lui
    argsnrm "x0,(3<<2)+1" ;#lui
    argsnrm "x0   ,   ( 2+(3< 4)  )(x2 )  "
}    

    proc chkadr {a {t i}} { 
        if {$t=="d"} { ;#d-data
            if {[info exists ::mem([a64 $a/8*8])]} {return 1}
            if {[info exists ::mem([a64 $a/8*8+4])]} {return 1}
        } else { ;#i-instr
            if {[info exists ::mem([a64 $a/4*4])]} {return 1}
            if {[info exists ::mem([a64 $a/8*8])] && "[lindex $::mem([a64 $a/8*8]) 0]"=="DD"} {return 1}
        }
        return 0
    }
    
#first pass, creates the ::sym table with all the labels
    proc asm {body} { ;#parse and create the ::mem array and the ::sym array
        catch {unset ::mem}; catch {unset ::sym}; regc; set ::PC [a64 0]; #good for repeated sourcing
        set ::res "\n" ;#progress before error
        foreach line [split $body \n] {
            foreach {label op args cm} [lns $line] {}
            set lineorg $line; set ::res "$::res$line\n" ;#progress before error
#  puts "asm: line=$line|\nlabel=$label| op=$op| args=$args|"            
            if {"$label"!=""} {
				if {![info exists ::sym($label)] || ($label=="START")} {
					set ::sym($label) [a64 $::PC]
				} else {
					error "redefined label"
				}
			}
			if {"$label"!=""} {
				if {![string is alpha [string index $label 0]] || ![string is wordchar "$label"]} {
					error "Label name must be alphanumeric starting with a letter"
				}
			}
            if {$op==""} {continue}
            set args [string trim $args "{}"]
            if {$op=="DC"}  {; #define characters e.g. DC "abc" converts to multiple DD ... 8 bytes each with added 0x00s at the end
				if {[string index $args 0]!="\"" || [string index $args end]!="\""} {
					error "DC operand must be enclosed in double quotes"
				}
				set args [subst -nocommands -novariables [string range $args 1 end-1]]
                set args [args2hex [split $args ""] a1]; set op DD ;# only 1 string allowed!
            #} elseif {$op=="DD"}  {; #can't do this because DD label will not work
            #    set args [args2hex [split $args ,] W1]; set op DD
            } elseif {$op=="DW"}  {; #define words e.g. DC "abc" converts to multiple DD ... 8 bytes each with added 0x00s at the end
                set args [args2hex [split $args ,] I1]; set op DD
            } elseif {$op=="DH"}  {; #define half-words e.g. DC "abc" converts to multiple DD ... 8 bytes each with added 0x00s at the end
                set args [args2hex [split $args ,] S1]; set op DD
            } elseif {$op=="DB"}  {; #define bytes e.g. DC 1,2,3 converts to multiple DD ... 8 bytes each with added 0x00s at the end
                set args [args2hex [split $args ,] c1]; set op DD
            } elseif {$op=="DF"}  {; #define 64-bit floats, e.g. DF 1.0 converts to DD 0x...
                set args [args2hex [split $args ,] Q1]; set op DD
            } elseif {$op=="DM"}  {; #define memory storage in number of DD 0s, e.g. DM 4 converts to DD 0,0,0,0
                set args [string map {" " ,} [lrepeat $args 0]]; set op DD
            }
            if {$op=="DD"}  {; #define doubles e.g. DD 1,-1,1000,0xff; DD label works, too (resolved in dump)
#puts "asm DD args=$args"
                set first 1
                foreach arg [split $args ,] {
                    set arg [string trim $arg " "] ;
                    if {$arg==""} {error "Missing operand"}
                    if {[expr $::PC%8]!=0} { ;#error "ERROR: Doubleword alignment problem"
                        set ::PC [expr $::PC/8*8+8]
                        if {"$label"!=""} {set ::sym($label) [a64 $::PC]}
                    } 
                    if {[chkadr $::PC d]} {error "Overwrite attempt"}
                    set res [list $op $arg]
                    if {$first} {lappend res $lineorg; set first 0}
                    set ::mem([a64 $::PC]) "$res"; set ::PC [a64 [incr ::PC 8]]
                }
                continue ;#next line
            }
            if {$op=="ORG"} {
                if {![string is wideinteger [set args [slbl $args]]]} {
                    error "Wrong ORG value/expr (no forward labels allowed)"
                }
                set ::PC [a64 [expr $args]]
                if {"$label"!=""} {set ::sym($label) [a64 $::PC]}
                continue
            }
            if {$op=="EQU"} {
                if {"$label"==""} {error "EQU must have a label"}
                if {![string is wideinteger [set args [slbl $args]]]} {
                    error "Wrong EQU value/expr (no forward labels allowed)"
                }
                if {"$label"!=""} {set ::sym($label) [a64 $args]}
                continue
            }
            if {[expr $::PC%4]!=0} { ;#error "ERROR: Word alignment problem" 
                set ::PC [expr $::PC/4*4+4]
                if {"$label"!=""} {set ::sym($label) [a64 $::PC]}
            } 
            if {[chkadr $::PC i]} {error "Overwrite attempt"}
            if {[array names ::sym START]==""} {set ::sym(START) [a64 $::PC]}; #first machine instruction to generate
            set args [argsnrm $args $op] ;# $x1,imm($x0) -> $x0,$x0,imm
 #       puts "asm: args=$args|"
           #set ::mem([a64 $::PC]) [format "{%s} {%s} {%s}" $op $args $lineorg]
            set ::mem([a64 $::PC]) [list $op $args $lineorg]
            set ::PC [a64 [incr ::PC 4]]
        }
        if {[array names ::sym START]==""} {set ::sym(START) 0xfffffffffffffffc}; #no machine instruction generated (last 4 byte cell in memory)
        set res [dump]  
        set ::PC [a64 $::sym(START)]
        return $res
    } ;#end asm
    
   
    proc fmti {s} {
        return [format "%-7s %-15s" [lindex $s 0] [lrange $s 1 end]]        
    }
        
    
    proc dump {} { ;#create the listing
        set res "ASSEMBLY LISTING\n"
        set title [list "1 18 ADDRESS" "$::CheckbuttonBINVar 40 {BIN/HEX CODE}" "$::CheckbuttonHEXCompVar 25 {        HEX OPERANDS}" \
                        "$::CheckbuttonINTCompVar 25 {        INT OPERANDS}" "$::CheckbuttonTXTCompVar 50 {      TEXT SOURCE}"]
        foreach el $title { foreach {flag len txt} $el {
#     puts "dump: flag=$flag len=$len txt=$txt"
            if {$flag} {append line "[format "%-*s " $len $txt]"}
        }}
#     puts "dump: line=$line"
     append res $line\n
        foreach i [lsort -integer [array names ::mem]] { ;#instruction lines
   set ::res "$res\n$::mem($i)\n"      
#            puts [format "%04d %s" $i $::mem($i)]
            set ::PC [a64 $i]; #needed for branches and other relative stuff
            set op [lindex $::mem($i) 0]; set args [lindex $::mem($i) 1]; set orgline [lindex $::mem($i) 2]
#puts "dump: op=$op| args=$args| ogline=$orgline|"            
            if {$op=="DD"} { ;#memory
#puts "dump args=$args"            
                #if {[info exists ::sym($args)]} {set args [format "%ld" [expr $::sym($args)]]} ;#symbol found in DD value is a label
                if {![string is wideinteger [set args [slbl $args]]]} {
                    set args "ERROR: NOT A NUMBER"
                } else {
                    set args [format "0x%016lx" $args]
                }
                set ::mem($i) [list $op $args $orgline]
                set fmt [format "0x%016lx" $i]            
                set res "$res$fmt"
                if {$::CheckbuttonBINVar} {set res "$res $op $args                   "}
                if {$::CheckbuttonHEXCompVar} {set res "$res [format "%-25s" {}]"} ;#instr-list
                if {$::CheckbuttonINTCompVar} {set res "$res [format "%-25s" {}]"} ;#instr-braces (?not shown)
                if {$::CheckbuttonTXTCompVar} {set res "$res [lns $orgline 1]"}
                set res "$res\n"
            } else { ;#instruction
                set instr1 [eval instr $op $args]; set asm [lindex $instr1 1]
#    puts "dump i=$i instr1=$instr1 asm=$asm $::mem($i)"            
                set ::mem($i) "[list [lindex $asm 0] [lrange $asm 1 end] [lindex $::mem($i) 2]]" 
                      #addr    instr-binary              instr-list             instr-braces        source-original
#                set fmt [format "0x%016x %-40s %-25s %-25s %s" $i [lindex $instr1 0] [fmti [lindex $instr1 1]] [fmti [lindex $instr1 2]] [lindex $::mem($i) 2]]
                set fmt [format "0x%016lx" $i] ;#address           
                set res "$res$fmt"
                if {$::CheckbuttonBINVar} {set res "$res [format "%-40s" [lindex $instr1 0]]"} ;#instr-binary
                if {$::CheckbuttonHEXCompVar} {set res "$res [format "%-25s" [fmti [lindex $instr1 1]]]"} ;#instr-list
                if {$::CheckbuttonINTCompVar} {set res "$res [format "%-25s" [fmti [lindex $instr1 2]]]"} ;#instr-braces (?not shown)
                if {$::CheckbuttonTXTCompVar} {set res "$res [lns $orgline 1]"} ;#source-original
                set res "$res\n"
            }
        }
        set res "$res\nSYMBOL TABLE\n"
        foreach i [lsort [array names ::sym]] { ;#symbol table
            set res "$res[format "0x%016lx %-10s" $::sym($i) $i]\n"
        }
    set ::res $res    
        return $res
    } ;#dump _sym

    
    
    proc memp {} { ;#print the memory (DD elements)
        set res "MEMORY\n"
        set title [list "1 18 ADDRESS" "$::CheckbuttonHEXVar 18 HEXADECIMAL" "$::CheckbuttonINTVar 20 INTEGER" \
                        "$::CheckbuttonFLPVar 14 FLOAT" "$::CheckbuttonTXTMemVar 50 {SOURCE TEXT}"]
        foreach el $title { foreach {flag len txt} $el {
#     puts "memp: flag=$flag len=$len txt=$txt"
            if {$flag} {append line "[format "%-*s " $len $txt]"}
        }}
#     puts "memp: line=$line"
     append res $line\n
        foreach i [lsort -integer [array names ::mem]] { ;#instruction lines
            if {[lindex $::mem($i) 0]=="DD"} {
                set line "$i" ;#address in hex
                set val [lindex $::mem($i) 1] ;#value in hex
                if {$::CheckbuttonHEXVar} {
                    set line "$line $val" ;#value in hex
                }
                if {$::CheckbuttonINTVar} { ;#value in int
                    if {[catch {set line "$line [format "%-20ld" $val]"} err]} {
                        set line "$line [format "%-20s" NOT_AN_INT]"
                    }
                }
                if {$::CheckbuttonFLPVar} { ;#value in float
                    if {[catch {set line "$line [format "%24.16E" [x2f $val]]"} err]} { ;#set res "$res [format "%24.16E" [x2f $reg]]\n"
                        set line "$line [format "%14s" NOT_A_FLP]"
                    }
                }
                if {$::CheckbuttonTXTMemVar} {
                    set line "$line [lns [lindex $::mem($i) 2] 1]"
                }
                set res "$res$line\n"
            }
        }
        return $res
    }
    
    
    proc reg2i {r} { ;#returns "" if r is not a valid register
        set ::x0 "0x0000000000000000"; #set ::f0 "0x0000000000000000"
        if {[string index $r 0]!="x" && [string index $r 0]!="f"} {
            set r [lindex $::regs [expr [lsearch $::regs $r]-1]]
        }
        set r [string range $r 1 end]
#   puts "reg2i: r=$r"   
        if {!([string is integer $r] && $r>=0 && $r<=31)} {set r ""}
#   puts "reg2i: r=$r"   
        return $r
    } 

    proc regc {} { ;#clear all registers
        for {set i 0} {$i<32} {incr i} {
            set ::x$i [format "0x%016lx" 0]
            set ::f$i [format "0x%016lx" 0]
        }
    }
    proc regp {} { ;#print all registers
        set res ""
        append res "INT Regs\n"; if {!$::CheckbuttonINTRegVar} {append res "---OFF---\n"}
        for {set i 0} {$i<32} {incr i} {
            if {$::CheckbuttonINTRegVar} { ;#INT
                set reg [set ::x$i]; set ralt [lindex $::regs [expr [lsearch $::regs x$i]+1]]            
                set res "$res[format "x%-2d %-4s" $i $ralt]"
                if {[string is wide $reg]} {            
                    if {$::CheckbuttonHEXRegVar} {
                        set res "$res $reg"
                    }
                    set res "$res [format "%-20ld" $reg]\n"
                } else { 
                    set res "$res $reg\n" ;#error value
                }
            }
        }
        append res "FLP Regs\n"; if {!$::CheckbuttonFLPRegVar} {append res "---OFF---\n"}
        for {set i 0} {$i<32} {incr i} {
            if {$::CheckbuttonFLPRegVar} { ;#FP
                set reg [set ::f$i] ;#0x0123456789abcdef            
                set res "$res[format "f%-2d" $i]"
                if {[string is double $reg]} {            
                    if {$::CheckbuttonHEXRegVar} {
                        set res "$res $reg"
                    }
#                    set res "$res [format "%24.16E" [x2f $reg]]\n"
                    if {[catch {set res "$res [format "%24.16E" [x2f $reg]]\n"} err]} { ;#set res "$res [format "%24.16E" [x2f $reg]]\n"
#                        set res "$res [format "%14s" NOT_A_FLP]\n"
                        set res "$res [format "%14s" [x2f $reg]]\n"
                    }
                } else { 
                    set res "$res $reg]\n" ;#error value
                }
            }
        }
        return $res
    } 

} ;;#kkdef end
#
if {1} { ;#instr all in this if

    proc instr {op args} { ;# no ::mem writing
    #instr add x0 x1 x2
#puts "args=$args"
        while {1} {
            set instrd [lsearch -index 0 -all -inline $::instrl $op] ;#search for the op code
            set instrd [lindex $instrd 0] ;#get the instr description sublist
            set instrop [lindex $instrd 2] ;#instr code
            set instrf [lindex $instrd 1] ;#instr function R/I/S/SB/U
            set err {}
################################            
            if {$instrf=="R"} { #add rd rs1 rs2; rd=rs1+rs2
                set instrf3 [lindex $instrd 3]; set instrf7 [lindex $instrd 4] ;#instr funct3, funct7
                set bin ""; set asmx "$op rd,rs1,rs2" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set rs1 [lindex $args 1]; set rs2 [lindex $args 2] ;#get the args
    #puts "R: rd=$rd rs1=$rs1 rs2=$rs2"
                set asm "$op $rd,$rs1,$rs2" ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]; set f7b [format "%07b" $instrf7]
                if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd  NOT A REGISTER"; break}
                if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]=="f"} {set err "ERROR: rs1 NOT A REGISTER"; break}
                if {[set rs2x [reg2i $rs2]]=="" || [string index $rs2 0]=="f"} {set err "ERROR: rs2 NOT A REGISTER"; break}
                set asmx "$op x$rdx x$rs1x x$rs2x" ;#asm (x-reg)
                set rdb  [format "%05b" $rdx]; set rs1b [format "%05b" $rs1x]; set rs2b [format "%05b" $rs2x]
                set bin "R  $f7b $rs2b $rs1b $f3b $rdb $opb" ;#binary instruction
                break
################################            
            } elseif {$instrf=="I"} { #addi rd rs1 imm; rd=rs1+imm; labels ok
    #ld rd imm(rs1); different order but uses () instead of ,
                set instrf3 [lindex $instrd 3]
                set bin ""; set asmx "$op rd,rs1,imm" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set rs1 [lindex $args 1]; set imm [lindex $args 2] ;#get the args
#  puts "I: rd=$rd rs1=$rs1 imm=$imm"
    if {$op!="ld"} {set asm "$op $rd,$rs1,$imm"} else {set asm "$op $rd,$imm\($rs1\)"} ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]
                if {$op=="ecall"} { ;#any register is ok
                    if {[set rdx  [reg2i $rd ]]==""} {set err "ERROR: rd  NOT A REGISTER"; break}
                    if {[set rs1x [reg2i $rs1]]==""} {set err "ERROR: rs1 NOT A REGISTER"; break}
                } else { ;#any register but f-register is ok
                    if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd  NOT A REGISTER"; break}
                    if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]=="f"} {set err "ERROR: rs1 NOT A REGISTER"; break}
                }
                #if {[array names ::sym $imm]==$imm} {set imm [format "%d" [expr $::sym($imm)]]} ;#symbol found
                #if {[string length $imm]==3 && [string index $imm 0]=="'" && [string index $imm 2]=="'" } {
                #    binary scan [string index $imm 1] H* hex; set imm "0x$hex" ;#single char literal e.g. 'a'
                #}
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
#                if {$imm<-4096 || $imm>4096-1} {set err "imm OUT OF RANGE \[-4096,4095\]"; break}
##                if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed
#                if {$imm<-2048} {set imm [expr $imm+4096]} ;#no need 
##                if {$imm<-2048 || $imm>2047} {set err "imm OUT OF RANGE \[-2048,2047\]"; break}
                set pw 11 ;#2**11=2048
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#12bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw+1)-1} {set err "ERROR: imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw+1))-1]\]"; break}
#                set val [i2x $imm]
#                if {[string range $val 2 14]=="0000000000000" || [string range $val 2 14]=="fffffffffffff"} {
#                    set imm "0x[string range $val 15 17]"
#                } else {
#                    set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw))-1]\]"; break
#                    set err "imm OUT OF RANGE"; break
#                }
    #            set asmx "$op x$rdx,$imm\(x$rs1x\)" ;#asm (x-reg)
                #set asmx "$op x$rdx x$rs1x [format "0x%x" $imm]" ;#asm (x-reg)
#                set asmx "$op x$rdx x$rs1x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
                set rdxp [string index $rd 0]; if {$rdxp!="f"} {set rdxp "x"} ;#if not f make it x
                set rs1xp [string index $rs1 0]; if {$rs1xp!="f"} {set rs1xp "x"} ;#if not f make it x
#          puts "instr: rdxp=$rdxp rs1xp=$rs1xp"      
                set asmx "$op $rdxp$rdx $rs1xp$rs1x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
#                set asmx "$op x$rdx x$rs1x $imm" ;#asm (x-reg)
                set rdb  [format "%05b" $rdx]; set rs1b [format "%05b" $rs1x]
                set immb [string range [format "%012b" $imm] end-11 end] ;# to shorten to 12 bits
                set bin "I  $immb  $rs1b $f3b $rdb $opb" ;#binary instruction
    if {$op!="ld"} {set asm "$op $rd,$rs1,[expr $imm]"} else {set asm "$op $rd,[expr $imm]\($rs1\)"} ;#asm code as should be entered
                break
################################            
            } elseif {$instrf=="S"} { #sd rs2 rs1 imm; mem(rs1+imm)=rs2; sd rs2 imm(rs1); labels ok
                set instrf3 [lindex $instrd 3]
                set bin ""; set asmx "$op rs2,rs1,imm" ;#init (shown in case of errors)
                set rs2  [lindex $args 0]; set rs1 [lindex $args 1]; set imm [lindex $args 2] ;#get the args
    #puts "S: rs2=$rs2 rs1=$rs1 imm=$imm"
    set asm "$op $rs2,$imm\($rs1\)" ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]
                if {[set rs2x  [reg2i $rs2]]=="" || [string index $rs2 0]=="f"} {set err "ERROR: rs2 NOT A REGISTER"; break}
                if {[set rs1x  [reg2i $rs1]]=="" || [string index $rs1 0]=="f"} {set err "ERROR: rs1 NOT A REGISTER"; break}
                #if {[array names ::sym $imm]==$imm} {set imm [format "%d" [expr $::sym($imm)]]} ;#symbol found
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
#                if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed
#                if {$imm<-2048 || $imm>2048-1} {set err "imm OUT OF RANGE \[-2048,2047\]"; break}
                set pw 11 ;#2**11=2048
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#12bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw+1)-1} {set err "ERROR: imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw+1))-1]\]"; break}
#                set asmx "$op x$rs2x x$rs1x [format "0x%x" $imm]" ;#asm (x-reg)
                set asmx "$op x$rs2x x$rs1x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
#                set asmx "$op x$rs2x x$rs1x $imm" ;#asm (x-reg)
                set rs2b [format "%05b" $rs2x]; set rs1b [format "%05b" $rs1x]
                set immb [string range [format "%012b" $imm] end-11 end] ;# to shorten to 12 bits
                set bin "S  [string range $immb [expr 11-11] [expr 11-5]] $rs2b $rs1b $f3b [string range $immb [expr 11-4] [expr 11-0]] $opb" ;#binary instruction
    set asm "$op $rs2,[expr $imm]\($rs1\)" ;#asm code as should be entered
                break
################################            
            } elseif {$instrf=="SB"} { #beq rs1 rs2 imm; if(rs1==rs2)PC=PC+imm; labels ok (difference calculated!)
    # imm is always an absolute address (constants, expr with labels, etc. are ok) from which the PC is subtracted         
                set instrf3 [lindex $instrd 3]
                set bin ""; set asmx "$op rs1,rs2,imm" ;#init (shown in case of errors)
                set rs1  [lindex $args 0]; set rs2 [lindex $args 1]; set imm [lindex $args 2] ;#get the args
#    puts "SB: rs1=$rs1 rs2=$rs2 imm=$imm PC=$::PC"
    set asm "$op $rs1,$rs2,$imm" ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]
                if {[set rs1x  [reg2i $rs1]]=="" || [string index $rs1 0]=="f"} {set err "ERROR: rs1 NOT A REGISTER"; break}
                if {[set rs2x  [reg2i $rs2]]=="" || [string index $rs2 0]=="f"} {set err "ERROR: rs2 NOT A REGISTER"; break}
                #if {[array names ::sym $imm]==$imm} { ;#symbol found
#    puts "SB: imm=$imm sym($imm)=$::sym($imm) PC=$::PC"
                #    set imm [format "%d" [expr ($::sym($imm)-$::PC)/2]]
                #}
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
                if {$imm<0} {set err "ERROR: imm IS NEGATIVE"; break}
                if {$imm%4!=0} {set err "ERROR: imm NOT WORD-ALIGNED"; break}
                set imm [expr ($imm-$::PC)] 
#                if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed MUST BE 13-bits???
#                if {$imm<-2048 || $imm>2048-1} {set err "imm OUT OF RANGE \[-2048,2047\]"; break}
                set pw 12 ;#2**12=4096
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#13bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {
                    set err "ERROR: imm OUT OF RANGE \[PC[expr int(-pow(2,$pw))],PC+[expr int(pow(2,$pw))-1]\]"; break
#                    set err "imm OUT OF RANGE"; break
                }
                set imm [expr $imm/2]
                set asmx "$op x$rs1x x$rs2x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
#                set asmx "$op x$rs1x x$rs2x $imm" ;#asm (x-reg)
                set rs1b [format "%05b" $rs1x]; set rs2b [format "%05b" $rs2x]
#                set immb [string range [format "%012b" $imm] end-11 end] ;# to shorten to 12 bits
                set immb [string range [format "%013b" $imm] end-12 end] ;# to shorten to 13 bits
    #puts "immb=$immb"
                #BA9876543210            
                #0123456789AB            
                set bin "SB [string index $immb [expr 12-12]][string range $immb [expr 12-10] [expr 12-5]] $rs2b $rs1b $f3b [string range $immb [expr 12-4] [expr 12-1]][string index $immb [expr 12-11]] $opb" ;#binary instruction
    set asm "$op $rs1,$rs2,[expr $imm]" ;#asm code as should be entered
                break
################################            
            } elseif {$instrf=="U"} { #lui rd imm; rd=imm (shifted 12 bits left); no labels
                set bin ""; set asmx "$op rd,imm" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set imm [lindex $args 1] ;#get the args
#    puts "U: rd=$rd imm=$imm"
    set asm "$op $rd,$imm" ;#asm code as should be entered
                if {[llength $args]!=2} {set err "ERROR: MUST HAVE 2 ARGS"; break}    
                set opb [format "%07b" $instrop];
                if {[set rdx  [reg2i $rd]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd NOT A REGISTER"; break}
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
                set pw 19 ;#2**19
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#20bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw+1)-1} {set err "ERROR: imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw+1))-1]\]"; break}
                set val [i2x $imm]
#                if {[string range $val 2 12]=="00000000000" || [string range $val 2 12]=="fffffffffff"} {
#                    set imm "0x[string range $val 13 17]"
#                } else {
 #                    set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw))-1]\]"; break
#                    set err "imm OUT OF RANGE"; break
#                }
#                set asmx "$op x$rdx $imm" ;#asm (x-reg)
#                set asmx "$op x$rdx [format "0x%x" $imm]" ;#asm (x-reg)
                set asmx "$op x$rdx 0x[string range [format "%05x" $imm] end-4 end]" ;#asm (x-reg)
                set rdb [format "%05b" $rdx]
                set immb [string range [format "%020b" $imm] end-19 end] ;# to shorten to 20 bits
                set bin "U  $immb $rdb $opb"
    set asm "$op $rd,[expr $imm]" ;#asm code as should be entered (INT Checkbutton)
                break
################################            
            } elseif {$instrf=="UJ"} { #jal rd imm; rd=PC+4 PC=PC+imm; labels ok
                set bin ""; set asmx "$op rd,imm" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set imm [lindex $args 1] ;#get the args
#    puts "UJ: rd=$rd imm=$imm"
    set asm "$op $rd,$imm" ;#asm code as should be entered
                if {[llength $args]!=2} {set err "ERROR: MUST HAVE 2 ARGS"; break}    
                set opb [format "%07b" $instrop];
                if {[set rdx  [reg2i $rd]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd NOT A REGISTER"; break}
#                if {[array names ::sym $imm]==$imm} {set imm [format "%d" [expr ($::sym($imm)-$::PC)/2]]} ;#symbol found
                #if {[array names ::sym $imm]==$imm} { ;#symbol found
#    puts "SB: imm=$imm sym($imm)=$::sym($imm) PC=$::PC"
                #    set imm [format "%d" [expr ($::sym($imm)-$::PC)/2]]
#    puts "SB: imm=$imm"
                #}
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
                if {$imm<0} {set err "ERROR: imm IS NEGATIVE"; break}
                if {$imm%4!=0} {set err "ERROR: imm NOT WORD-ALIGNED"; break}
                set imm [expr ($imm-$::PC)] 
                #if {$imm> pow(2,20)-1} {set imm [expr $imm-pow(2,21)]} ;#20bit unsigned to signed
                #if {$imm<-pow(2,20) || $imm>pow(2,20)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,20))],[expr int(pow(2,20)-1)]\]"; break}
                set pw 21 ;#2**21
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#21bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {
                    set err "ERROR: imm OUT OF RANGE \[PC[expr int(-pow(2,$pw))],PC+[expr int(pow(2,$pw))-1]\]"; break
#                    set err "imm OUT OF RANGE"; break
                }
                set imm [expr $imm/2]
#                set asmx "$op x$rdx $imm" ;#asm (x-reg)
#                set asmx "$op x$rdx [format "0x%x" $imm]" ;#asm (x-reg)
                set asmx "$op x$rdx 0x[string range [format "%05x" $imm] end-4 end]" ;#asm (x-reg)
#    puts "asmx=$asmx"        
                set rdb [format "%05b" $rdx]
                set immb [string range [format "%021b" $imm] end-20 end] ;# to shorten to 21 bits
                set bin "UJ [string index $immb [expr 20-20]][string range $immb [expr 20-10] [expr 20-1]][string index $immb [expr 20-11]][string range $immb [expr 20-19] [expr 20-12]] $rdb $opb"
    set asm "$op $rd,[expr $imm]" ;#asm code as should be entered
                break
                
                
################################            
            } elseif {$instrf=="F2"} { #float: fcvt.l.d fd rs1, fd=double(rs1); fcvt.d.l rd fs1; rd=round(fs1)
                set instrf3 [lindex $instrd 3]; set instrf7 [lindex $instrd 4] ;#instr funct3, funct7
                set bin ""; set asmx "$op fd,fs1,fs2" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set rs1 [lindex $args 1]; #set rs2 [lindex $args 2] ;#get the args
                if {$op in "fcvt.d.l fcvt.l.d"} {
					set rs2 f2 
				} elseif {$op in "fcvt.d.lu fcvt.lu.d"} {
					set rs2 f3
				} else {
					set rs2 f0 ;#fsqrt.d, fmv.x.d, fmx.d.x
				}
#    puts "F2: fd=$rd fs1=$rs1 fs2=$rs2"
                set asm "$op $rd,$rs1,$rs2" ;#asm code as should be entered
                if {[llength $args]!=2} {set err "ERROR: MUST HAVE 2 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]; set f7b [format "%07b" $instrf7]
#                if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]!="f"} {set err "fd  NOT AN FP REGISTER"; break}
                if {!($op in "fcvt.d.l fcvt.d.lu fmv.d.x fsqrt.d")} { ;#rd is integer register 2019.04.28
                    if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd  NOT A REGISTER"; break}
                    if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]!="f"} {set err "ERROR: fs1 NOT AN FP REGISTER"; break}
                } else { ;#rd is f-register
                    if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]!="f"} {set err "ERROR: fd  NOT AN FP REGISTER"; break}
					if {!($op in "fsqrt.d")}	{				
						if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]=="f"} {set err "ERROR: rs1 NOT A REGISTER"; break}
					} else {
						if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]!="f"} {set err "ERROR: rs1 NOT A REGISTER"; break} ;#fsqrt.d
					}
                }
                if {[set rs2x [reg2i $rs2]]=="" || [string index $rs2 0]!="f"} {set err "ERROR: fs2 NOT AN FP REGISTER"; break}
                set asmx "$op [string index $rd 0]$rdx [string index $rs1 0]$rs1x f$rs2x" ;#asm (x-reg) 2019.04.28
                set rdb  [format "%05b" $rdx]; set rs1b [format "%05b" $rs1x]; set rs2b [format "%05b" $rs2x]
                set bin "FR $f7b $rs2b $rs1b $f3b $rdb $opb" ;#binary instruction
                break
################################            
            } elseif {$instrf=="FR"} { #float: fadd.d fd fs1 fs2; fd=fs1+fs2
                set instrf3 [lindex $instrd 3]; set instrf7 [lindex $instrd 4] ;#instr funct3, funct7
                set bin ""; set asmx "$op fd,fs1,fs2" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set rs1 [lindex $args 1]; set rs2 [lindex $args 2] ;#get the args
#    puts "FR: fd=$rd fs1=$rs1 fs2=$rs2"
                set asm "$op $rd,$rs1,$rs2" ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]; set f7b [format "%07b" $instrf7]
#                if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]!="f"} {set err "fd  NOT AN FP REGISTER"; break}
                if {$op in "feq.d flt.d fle.d"} { ;#rd is integer register
                    if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]=="f"} {set err "ERROR: rd NOT A REGISTER"; break}
                } else { ;#rd is f-register
                    if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]!="f"} {set err "ERROR: fd NOT AN FP REGISTER"; break}
                }
                if {[set rs1x [reg2i $rs1]]=="" || [string index $rs1 0]!="f"} {set err "ERROR: fs1 NOT AN FP REGISTER"; break}
                if {[set rs2x [reg2i $rs2]]=="" || [string index $rs2 0]!="f"} {set err "ERROR: fs2 NOT AN FP REGISTER"; break}
                set asmx "$op [string index $rd 0]$rdx f$rs1x f$rs2x" ;#asm (x-reg)
                set rdb  [format "%05b" $rdx]; set rs1b [format "%05b" $rs1x]; set rs2b [format "%05b" $rs2x]
                set bin "FR $f7b $rs2b $rs1b $f3b $rdb $opb" ;#binary instruction
                break
################################            
            } elseif {$instrf=="FI"} { #float fld fd rs1 imm; fd=mem(rs1+imm); fld fd,imm(rs1); labels ok
    #ld rd imm(rs1); different order but uses () instead of ,
                set instrf3 [lindex $instrd 3]
                set bin ""; set asmx "$op fd,rs1,imm" ;#init (shown in case of errors)
                set rd  [lindex $args 0]; set rs1 [lindex $args 1]; set imm [lindex $args 2] ;#get the args
#    puts "FI: rd=$rd rs1=$rs1 imm=$imm"
    if {$op!="fld"} {set asm "$op $rd,$rs1,$imm"} else {set asm "$op $rd,$imm\($rs1\)"} ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]
                if {[set rdx  [reg2i $rd ]]=="" || [string index $rd 0]!="f"} {set err "ERROR: fd NOT AN FP REGISTER"; break}
                if {[set rs1x [reg2i $rs1]]==""} {set err "ERROR: rs1 NOT A REGISTER"; break}
                #if {[array names ::sym $imm]==$imm} {set imm [format "%d" [expr $::sym($imm)]]} ;#symbol found
                #if {[string length $imm]==3 && [string index $imm 0]=="'" && [string index $imm 2]=="'" } {
                #    binary scan [string index $imm 1] H* hex; set imm "0x$hex"
                #}
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
#                if {$imm<-4096 || $imm>4096-1} {set err "imm OUT OF RANGE \[-4096,4095\]"; break}
##                if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed
#                if {$imm<-2048} {set imm [expr $imm+4096]} ;#no need 
##                if {$imm<-2048 || $imm>2047} {set err "imm OUT OF RANGE \[-2048,2047\]"; break}
                set pw 11 ;#2**11=2048
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#12bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw+1)-1} {set err "ERROR: imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw+1))-1]\]"; break}
                #            set asmx "$op x$rdx,$imm\(x$rs1x\)" ;#asm (x-reg)
#                set asmx "$op x$rdx x$rs1x [format "0x%x" $imm]" ;#asm (x-reg)
#                set asmx "$op f$rdx x$rs1x $imm" ;#asm (x-reg)
                set asmx "$op f$rdx x$rs1x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
                set rdb  [format "%05b" $rdx]; set rs1b [format "%05b" $rs1x]
                set immb [string range [format "%012b" $imm] end-11 end] ;# to shorten to 12 bits
                set bin "FI $immb  $rs1b $f3b $rdb $opb" ;#binary instruction
    if {$op!="fld"} {set asm "$op $rd,$rs1,[expr $imm]"} else {set asm "$op $rd,[expr $imm]\($rs1\)"} ;#asm code as should be entered
                break
################################            
            } elseif {$instrf=="FS"} { #float: fsd fs2 rs1 imm; mem(rs1+imm)=fs2; fsd rs2,imm(rs1); labels ok
                set instrf3 [lindex $instrd 3]
                set bin ""; set asmx "$op fs2,rs1,imm" ;#init (shown in case of errors)
                set rs2  [lindex $args 0]; set rs1 [lindex $args 1]; set imm [lindex $args 2] ;#get the args
#    puts "FS: rs2=$rs2 rs1=$rs1 imm=$imm"
    set asm "$op $rs2,$imm\($rs1\)" ;#asm code as should be entered
                if {[llength $args]!=3} {set err "ERROR: MUST HAVE 3 ARGS"; break}    
                set opb [format "%07b" $instrop]; set f3b [format "%03b" $instrf3]
                if {[set rs2x  [reg2i $rs2]]=="" || [string index $rs2 0]!="f"} {set err "ERROR: fs2 NOT AN FP REGISTER"; break}
                if {[set rs1x  [reg2i $rs1]]==""} {set err "ERROR: rs1 NOT A REGISTER"; break}
                #if {[array names ::sym $imm]==$imm} {set imm [format "%d" [expr $::sym($imm)]]} ;#symbol found
                if {![string is integer [set imm [slbl $imm]]]} {set err "ERROR: imm NOT AN INTEGER"; break}
#                if {$imm<-4096 || $imm>4096-1} {set err "imm OUT OF RANGE \[-4096,4095\]"; break}
##                if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed
#                if {$imm<-2048} {set imm [expr $imm+4096]} ;#no need 
##                if {$imm<-2048 || $imm>2047} {set err "imm OUT OF RANGE \[-2048,2047\]"; break}
                set pw 11 ;#2**11=2048
#                if {$imm> pow(2,$pw)-1} {set imm [expr $imm-int(pow(2,$pw+1))]} ;#12bit unsigned to signed
#                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw)-1} {set err "imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw)-1)]\]"; break}
                if {$imm<-pow(2,$pw) || $imm>pow(2,$pw+1)-1} {set err "ERROR: imm OUT OF RANGE \[[expr int(-pow(2,$pw))],[expr int(pow(2,$pw+1))-1]\]"; break}
#                set asmx "$op x$rs2x x$rs1x [format "0x%x" $imm]" ;#asm (x-reg)
#                set asmx "$op f$rs2x x$rs1x $imm" ;#asm (x-reg)
                set asmx "$op f$rs2x x$rs1x 0x[string range [format "%03x" $imm] end-2 end]" ;#asm (x-reg)
                set rs2b [format "%05b" $rs2x]; set rs1b [format "%05b" $rs1x]
                set immb [string range [format "%012b" $imm] end-11 end] ;# to shorten to 12 bits
                set bin "FS [string range $immb [expr 11-11] [expr 11-5]] $rs2b $rs1b $f3b [string range $immb [expr 11-4] [expr 11-0]] $opb" ;#binary instruction
    set asm "$op $rs2,[expr $imm]\($rs1\)" ;#asm code as should be entered
                break
                
################################            
            } else {
                set bin ""; set asm "$op $args"
                set asmx "$op $args" ;#init (shown in case of errors)
                set err "ERROR: INSTR CODE NOT RECOGNIZED"; break
            }
        }
#puts "\{$bin\} \{$asm\} \{$err\}"
        if {$bin==""} {set bin $err}
        return "[list $bin $asmx $asm $err]"
    } ;#instr end
#instr add a0 a1 a2
#instr addi a0 a1 10
} ;#instr end
#
if {1} { ;#kkcmd all in this if
#----------------- "machine opcodes" implemented as procs
    set ::regs {
        x0  zero
        x1  ra
        x2  sp
        x3  gp
        x4  tp
        x5  t0
        x6  t1
        x7  t2
        x8  s0
        x8  fp
        x9  s1
        x10 a0
        x11 a1
        x12 a2
        x13 a3
        x14 a4
        x15 a5
        x16 a6
        x17 a7
        x18 s2
        x19 s3
        x20 s4
        x21 s5
        x22 s6
        x23 s7
        x24 s8
        x25 s9
        x26 s10
        x27 s11
        x28 t3
        x29 t4
        x30 t5
        x31 t6
    }
#                       op      f3      f7
    set ::instrl {
        {lb         I   0x3     0x0}
        {lh         I   0x3     0x1}
        {lw         I   0x3     0x2}
        {ld         I   0x3     0x3}
        {lbu        I   0x3     0x4}
        {lhu        I   0x3     0x5}
        {lwu        I   0x3     0x6}
        {fence      I   0xf     0x0}
        {fence.i    I   0xf     0x1}
        {addi       I   0x13    0x0}
        {slli       I   0x13    0x1     0x0}
        {slti       I   0x13    0x2}
        {sltiu      I   0x13    0x3}
        {xori       I   0x13    0x4}
        {srli       I   0x13    0x5     0x0}
        {srai       I   0x13    0x5     0x20}
        {ori        I   0x13    0x6}
        {andi       I   0x13    0x7}
        {auipc      U   0x17}
        {addiw      I   0x1b    0x0}
        {slliw      I   0x1b    0x1     0x0}
        {srliw      I   0x1b    0x5     0x0}
        {sraiw      I   0x1b    0x5     0x20}
        {sb         S   0x23    0x0}
        {sh         S   0x23    0x1}
        {sw         S   0x23    0x2}
        {sd         S   0x23    0x3}
        {add        R   0x33    0x0     0x0}
        {sub        R   0x33    0x0     0x20}
        {sll        R   0x33    0x1     0x0}
        {slt        R   0x33    0x2     0x0}
        {sltu       R   0x33    0x3     0x0}
        {xor        R   0x33    0x4     0x0}
        {srl        R   0x33    0x5     0x0}
        {sra        R   0x33    0x5     0x20}
        {or         R   0x33    0x6     0x0}
        {and        R   0x33    0x7     0x0}
        {lui        U   0x37}
        {addw       R   0x3b    0x0     0x0}
        {subw       R   0x3b    0x0     0x20}
        {sllw       R   0x3b    0x1     0x0}
        {srlw       R   0x3b    0x5     0x0}
        {sraw       R   0x3b    0x5     0x20}
        {beq        SB  0x63    0x0}
        {bne        SB  0x63    0x1}
        {blt        SB  0x63    0x4}
        {bge        SB  0x63    0x5}
        {bltu       SB  0x63    0x6}
        {bgeu       SB  0x63    0x7}
        {jalr       I   0x67    0x0}
        {jal        UJ  0x6f}
        {ecall      I   0x73    0x0     0x0}
        {ebreak     I   0x73    0x0     0x1}
        {CSRRW      I   0x73    0x1}
        {CSRRS      I   0x73    0x2}
        {CSRRC      I   0x73    0x3}
        {CSRRWI     I   0x73    0x4}
        {CSRRSI     I   0x73    0x5}
        {CSRRCI     I   0x73    0x6}
        
        {mul        R   0x33    0x0     0x1}
        {mulh       R   0x33    0x1     0x1}
        {mulhsu     R   0x33    0x2     0x1}
        {mulhu      R   0x33    0x3     0x1}
        {div        R   0x33    0x4     0x1}
        {divu       R   0x33    0x5     0x1}
        {rem        R   0x33    0x6     0x1}

        {fld        FI  0x7     0x3}           
        {fsd        FS  0x27    0x3}           
        {fadd.d     FR  0x53    0x0     0x1}           
        {fsub.d     FR  0x53    0x0     0x5}           
        {fmul.d     FR  0x53    0x0     0x9}           
        {fdiv.d     FR  0x53    0x0     0xd} 
		
		{fsqrt.d    F2  0x53    0x0     0b0101101} 	
		{fsgnj.d    FR  0x53    0b000   0b0010001} 	
		{fsgnjn.d   FR  0x53    0b001   0b0010001} 	
		{fsgnjx.d   FR  0x53    0b010   0b0010001} 	
 		{fmin.d     FR  0x53    0b000   0b0010101} 	
		{fmax.d     FR  0x53    0b010   0b0010101} 	
       
        {feq.d      FR  0x53    0b010   0b1010001}           
        {flt.d      FR  0x53    0b001   0b1010001}           
        {fle.d      FR  0x53    0b000   0b1010001}
        
        {fcvt.l.d   F2  0x53    0x0     0b1100001}           
        {fcvt.lu.d  F2  0x53    0x0     0b1100001}           
        {fmv.x.d  	F2  0x53    0x0     0b1110001}           

        {fcvt.d.l   F2  0x53    0x0     0b1101001}           
        {fcvt.d.lu  F2  0x53    0x0     0b1101001}           
        {fmv.d.x  	F2  0x53    0x0     0b1111001}           

}
#                       op      f3      f7
#                       op      rm(RoundMode) f7 (for float)
#                       op      rs2(for fcvt) f7 (for float)
    if {0} {
        proc ADD  {reg reg2}  {set ::Z [incr ::$reg [set ::$reg2]]}
        proc ADI  {reg value} {set ::Z [incr ::$reg $value]}
        proc CALL {name}      {[string tolower $name] $::A}
        proc DCR  {reg}       {set ::Z [incr ::$reg -1]}
        proc INR  {reg}       {set ::Z [incr ::$reg]}
        proc JMP  where       {uplevel 1 set pc [expr $where-1]}
        proc JNZ  where       {if $::Z {uplevel 1 JMP $where}}
        proc JZ   where       {if !$::Z {uplevel 1 JMP $where}}
        proc MOV  {reg adr}   {variable mem; set ::$reg $mem($adr)}
        proc MVI  {reg value} {set ::$reg $value}
        proc ADD   {rd rs rt}    {set ::$rd [expr [set ::$rs]+[set ::$rt]]}
        proc ADDI  {rt rs imm}   {set ::$rt [expr [set ::$rs]+$imm]}
        proc BEQ   {rt rs imm}   {if ::$rt==::$rs {uplevel 1 j $imm}}
        proc J     {addr}        {uplevel 1 set pc [expr $addr-1]}
        proc LW    {rt rs imm}   {variable mem; set ::$rt $mem([expr [set ::$rs]+$imm])}
        proc SW    {rt rs imm}   {variable mem; set mem([expr [set ::$rs]+$imm]) [set ::$rt]}
    }    

#proc rg {r} {set ::x[reg2i $rs1]]}
#    proc ADD    {rd  rs1 rs2}   {set ::x[reg2i $rd] 0x[format "%016lX" [expr [rg $rs1]+[rg $rs2]]]}
#    proc ADD    {rd  rs1 rs2 op}   {set ::x[reg2i $rd] 0x[format "%016lX" [expr [rg $rs1]+[rg $rs2]]]}
    
    proc typeR  {rd  rs1 rs2 op {u 0}}   {
        if {$u} { ;#unsigned
            set ::x[reg2i $rd] 0x[format "%016lX" [expr [set ::x[reg2i $rs1]]$op[set ::x[reg2i $rs2]]]]
        } else {
            set ::x[reg2i $rd] 0x[format "%016lX" [expr [format %ld [set ::x[reg2i $rs1]]]$op[format %ld [set ::x[reg2i $rs2]]]]]
        }
    }
    proc typeI  {rd  rs1 imm op {u 0}}   {
#        puts "u=$u"
        if {$u} { ;#unsigned
            #if {$imm> 2047} {set imm [expr $imm-4096]} ;#12bit unsigned to signed
            if {$imm<0} {set imm [expr 4096+$imm]} ;#12bit signed to unsigned
            #puts "imm$imm"
            set ::x[reg2i $rd] 0x[format "%016lX" [expr [set ::x[reg2i $rs1]]$op$imm]]
        } else {
#            set ::x[reg2i $rd] 0x[format "%016lX" [expr [format %ld [set ::x[reg2i $rs1]]]$op[format %d $imm]]]
            set ::x[reg2i $rd] 0x[format "%016lX" [expr [format %ld [set ::x[reg2i $rs1]]]$op[x12i $imm]]]
        }
    }
    proc ADD    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2  + }
    proc ADDI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2  + }
    proc SUB    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2  - }
    proc SUBI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2  - }
    proc AND    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2  & }
    proc ANDI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2  & }
    proc OR     {rd  rs1 rs2}   {typeR $rd $rs1 $rs2  | }
    proc ORI    {rd  rs1 rs2}   {typeI $rd $rs1 $rs2  | }
    proc XOR    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2  ^ }
    proc XORI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2  ^ }
    proc SLL    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2 << }
    proc SLLI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2 << }
    proc SRL    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2 >> 1 } ;#unsigned
    proc SRLI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2 >> 1 } ;#unsigned
    proc SRA    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2 >> }
    proc SRAI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2 >> }
    
    proc SLT    {rd  rs1 rs2}   {typeR $rd $rs1 $rs2 <  }
    proc SLTI   {rd  rs1 rs2}   {typeI $rd $rs1 $rs2 <  }
    proc SLTU   {rd  rs1 rs2}   {typeR $rd $rs1 $rs2 <  1 } ;#unsigned
    proc SLTIU  {rd  rs1 rs2}   {typeI $rd $rs1 $rs2 <  1 } ;#unsigned

    proc Smem  {rs1 rs2 imm nbytes }  {
        set a [a64 [set ::x[reg2i $rs2]]+[x12i $imm]]
        set val [string range [set ::x[reg2i $rs1]] [expr 17-$nbytes*2+1] 17]
        memw $a $val
#  puts "Smem: a=$a val=$val"    
    }
    proc SD    {rd rs1 rs2}   {Smem $rd $rs1 $rs2 8  }
    proc SW    {rd rs1 rs2}   {Smem $rd $rs1 $rs2 4  }
    proc SH    {rd rs1 rs2}   {Smem $rd $rs1 $rs2 2  }
    proc SB    {rd rs1 rs2}   {Smem $rd $rs1 $rs2 1  }
    
    proc Lmem  {rd rs1 imm nbytes {u 0}}  {
#        set a [a64 [set ::x[reg2i $rs1]]+$imm]
        set a [a64 [set ::x[reg2i $rs1]]+[x12i $imm]]
        set w "[memr $a $nbytes]" ;#0 if never written (silent, no error)
#  puts "Lmem: rs1=[set ::x[reg2i $rs1]] imm=$imm"    
#  puts "Lmem: a=$a w=$w"    
#        if { !$u && ([string index [format %b 0x$w] 0]=="1") }  ;#sign extend
        if { !$u && ([string index [format %0[expr $nbytes*8]b 0x$w] 0]=="1") } { ;#sign extend
            set w [string range "ffffffffffffffff$w" end-17 end]
        }
        set w 0x$w
#  puts "Lmem: a=$a w=$w"    
        set ::x[reg2i $rd] [a64 $w]
    }
    proc LD    {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 8  }
    proc LW    {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 4  }
    proc LWU   {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 4  1 } ;#unsigned
    proc LH    {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 2  }
    proc LHU   {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 2  1 } ;#unsigned
    proc LB    {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 1  }
    proc LBU   {rd  rs1 rs2}   {Lmem $rd $rs1 $rs2 1  1 } ;#unsigned
    
    proc typeSB  {rs1  rs2 imm op {u 0}}   {
#        puts "u=$u"
        if {$u} { ;#unsigned
            set res 0x[format "%016lX" [expr [set ::x[reg2i $rs1]]$op[set ::x[reg2i $rs2]]]]
        } else {
            set res 0x[format "%016lX" [expr [format %ld [set ::x[reg2i $rs1]]]$op[format %ld [set ::x[reg2i $rs2]]]]]
        }
        if {$res} { ;#branch taken
#            set ::PC [a64 $::PC+([format "%d" $imm]<<1)-4] ;#must be "%d" and NOT "%ld"
            set ::PC [a64 $::PC+([x12i $imm]<<1)-4] ;#must be "%d" and NOT "%ld"
        }
    }
    proc BEQ    {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm == }
    proc BNE    {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm != }
    proc BGE    {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm >= }
    proc BGEU   {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm >= 1 } ;#unsigned
    proc BLT    {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm <  }
    proc BLTU   {rs1 rs2 imm}   {typeSB $rs1 $rs2 $imm <  1 } ;#unsigned
    
    proc JAL    {rd  imm    }   {
        set ::x[reg2i $rd] 0x[format "%016lX" [expr $::PC+0]];  
#        set ::PC [a64 $::PC+([format "%d" $imm]<<1)-4]
        set ::PC [a64 $::PC+([format "%ld" [x20i $imm]]<<1)-4]
    }
    #JALR is I-type so no offset calculation as in branch (SB-type), no shifts! Absolute jump to rs1+imm
    proc JALR   {rd  rs1 imm}   {set ::x[reg2i $rd] 0x[format "%016lX" [expr $::PC+0]];  set ::PC [a64 [set ::x[reg2i $rs1]]+([format "%ld" $imm]<<0)-0]}

#    proc LUI    {rd  imm} {set ::x[reg2i $rd] [a64 ([format "%ld" $imm]<<12)] }
    proc LUI    {rd  imm} {set ::x[reg2i $rd] [a64 ([format "%ld" [x20i $imm]]<<12)] }
#    proc AUIPC  {rd  imm} {set ::x[reg2i $rd] [a64 $::PC+([format "%ld" $imm]<<12)] }
    proc AUIPC  {rd  imm} {set ::x[reg2i $rd] [a64 $::PC+([format "%ld" [x20i $imm]]<<12)] }

#    proc EBREAK {rd rs1 imm}   {set ::PC [expr $::PC+4]; error "ebreak [i2x [expr $::PC-8]]\n"}
    proc EBREAK {rd rs1 imm}   {set ::PC [expr $::PC+4]; error "ebreak\n"}
    
proc getstr1 {a} { ;#get a null-terminated string starting at address a at double-word boundary
    if {$a%8!=0} {error "ERROR: source string must start at double-word boundary"}
    while {[string length [set s [getstr $a]]]==8} {incr a 8; append r $s}
    append r $s
    return $r
}    
proc getstr {a} {
    set a [a64 $a]
    if {![info exists ::mem($a)] || "[lindex $::mem($a) 0]"!="DD"} {return ""}
    set x [lindex $::mem($a) 1];
#    set x $a ;#for testing
    set rx [revx $x]
#    set f [string first "00" $rx]
    set s [x2s $rx]
    set f [string first "\0" $s]
#puts "getstr: a=$a x=$x f=$f rx=$rx"
    if {$f!=-1} {
        set e [expr [string length $s]-(10-$f)+1]
#puts "getstr: e=$e"
        set s [string range $s 0 $e]
    }
#puts "getstr: s=$s|"
    return $s
} 
if {0} {
    string length [getstr 0x0000000000000031]
    string length [getstr 0x0000000000003231]
    string length [getstr 0x0000000000333231]
    string length [getstr 0x3837363534333231]
}  
 
proc putstr {a s} { ;#put a null-terminated string starting at address a at double-word boundary
    set aa $a
    if {$a%8!=0} {error "ERROR: destination string must start at double-word boundary"}
#  puts "putstr1: a=$a s=$s"
    set s [args2hex [split $s ""] a1]
#  puts "putstr2: a=$a s=$s"
    foreach dw [split $s ","] {
#  puts "putstr3: dw=$dw"
        if {![info exists ::mem([a64 $a])] || "[lindex $::mem([a64 $a]) 0]"=="DD"} {
            set ::mem([a64 $a]) "DD $dw {}"
            incr a 8
        } else {
            return "ERROR: attempt to overwrite code at [a64 $a]"
        }
    }
#    return ""
    return "[a64 $aa]:$s"
}

    proc ECALL  {rd rs1 imm}   {
        global widget
# 'I'/'F'/'X'/'C'/'S', 0-ready 
#  0   1   2   3   4 output data->rs1 ('S'-start addrs in rs1) , code->imm
#  5   6   7   8   9 input  code->imm ('S'-start addrs in rs1) , data->rd
#codes imm = 0 print_int, 1 print double, 2 print hex (unsigned!), 3 print chars, 4 print 0string
        set rs1a [set ::[string index $rs1 0][reg2i $rs1]]
        set rda [set ::[string index $rd 0][reg2i $rd]]
#                3 {set res [x2s [revx $rs1a]]}
        if {$imm>=0 && $imm<=4} { ;#output
            switch -- [expr $imm] {
                x0 {set res [format "%-20ld" $rs1a]} 
                0 {set res [format "%ld" $rs1a]} 
                1 {set res [x2f $rs1a]}
                2 {set res $rs1a}
                3 {set res [string trim [x2s [revx $rs1a]] \0]}
                x4 {set res "WARNING: NOT IMPLEMENTED SYSCALL ([expr $imm])"}
                4 {set res [getstr1 $rs1a]}
                default {set res "ERROR: WRONG SYSCALL ([expr $imm])"}
            }
#            $::widget(TextOutput) insert end "$res\n"; $::widget(TextOutput) see end-1line
#puts "rd=$rd rda=$rda"
			if {$rd=="x0" || $rd=="zero"} { ;#new line only if rd is x0 or zero 2019.04.28
				kktxtw TextOutput "$res\n" 0 1
			} else {
				kktxtw TextOutput "$res" 0 1
			}
        } elseif {$imm>=5 && $imm<=9} { ;#input
#            $::widget(TextOutput) insert end "[x2s [revx $rs1a]]?"; $::widget(TextOutput) see end-1line
#            kktxtw TextOutput "[x2s [revx $rs1a]]?" 0 1
#            kktxtw TextOutput "[string trim [x2s [revx $rs1a]] \0]?" 0 1
            kktxtw TextOutput "[string trim [x2s [revx $rs1a]] \0]" 0 1
            if {[info exists ::inl]} { ;#Batch
                set ::ENTER 1
                set ::EntryInputVar [lindex $::inl 0]
                set ::inl [lrange $::inl 1 end]
            } else {
                grab $widget(FrameInput)
                focus $widget(EntryInput)
                blink
                set ::ENTER 0; vwait ::ENTER
                blink 0 
                grab release $widget(FrameInput)
            }            
            if {$::ENTER==1} {
#                $::widget(TextOutput) insert end "$::EntryInputVar\n"; $::widget(TextOutput) see end-1line
                kktxtw TextOutput "$::EntryInputVar\n" 0 1
                switch -- [expr $imm] {
                    5 {set res [i2x $::EntryInputVar]; set ::x[reg2i $rd] $res} 
                    6 {set res [f2x $::EntryInputVar]; set ::[string index $rd 0][reg2i $rd] $res}
                    7 {set res [a64 $::EntryInputVar]; set ::x[reg2i $rd] $res}
                    8 {set res [revx [s2x $::EntryInputVar]]; set ::x[reg2i $rd] $res}
                    x9 {set res "WARNING: NOT IMPLEMENTED SYSCALL ([expr $imm])"}
                    9 {set res [putstr $rda "$::EntryInputVar\0"]}
                    default {set res "ERROR: WRONG SYSCALL ([expr $imm])"}
                }
            } else { ;# 2 is Cancel
                    set res "Cancelled"
#                    $::widget(TextOutput) insert end "$res\n"; $::widget(TextOutput) see end-1line
                    kktxtw TextOutput "$res\n" 0 1
                    error "Cancelled"
            }
            set ::EntryInputVar ""
#            $::widget(TextOutput) insert end "$res\n"; $::widget(TextOutput) see end-1line
#            kktxtw TextOutput "$res\n" 0 1
        }
#    https://wiki.tcl.tk/9172
        #show the prompts
#    $::widget(TextOutput) insert end "$res\n"; $::widget(TextOutput) see end-1line
#    $::widget(TextOutput) insert end "[expr [set ::x[reg2i $rs1]]]?"; $::widget(TextOutput) see end-1line
#read the value
#     vwait ::EntryInputVar   
#    error "ecall at $::PC"
    }

    
    proc typeRM  {rd  rs1 rs2 op {u1 0} {u2 0} {h 0}}   {
puts "typeRM:rd=$rd rs1=$rs1 rs2=$rs2 op=$op u1=$u1 u2=$u2 h=$h"
        set rs1 [set ::x[reg2i $rs1]]; set rs2 [set ::x[reg2i $rs2]] ;#hex values from regs
#puts "typeRM-beg:rd=$rd rs1=$rs1 rs2=$rs2 op=$op u1=$u1 u2=$u2 h=$h"
        if {!$u1} { set rs1 [format %ld $rs1] } ;#rs1 signed
		if {!$u2} { set rs2 [format %ld $rs2] } ;#rs2 signed
        set res [expr $rs1 $op $rs2] ;#signed
#puts "typeRM1:rs1=$rs1 rs2=$rs2 res=$res"
		set hr [expr $res>>64]
		set lr [expr $res&0xffffffffffffffff]
#puts "typeRM2:hr=$hr lr=$lr"
puts "typeRM3:hrlr=[i2xn $res 32] [expr ($hr<<64)+$lr]" ;#128 bit
        set resh "[format 0x%016lx $hr]"
        set res  "[format 0x%016lx $lr]"
#         set res [format "%lld" $res] ;#signed
#puts "typeRM4:resh=$resh res=$res"
#        set res 0x[format "%032llX" $res] ;#don't work, the sign is infront of the hex for ll!!! (see proc i2xn)
        if {$h} {
#            set ::x[reg2i $rd] [string range $res 0 17]
            set ::x[reg2i $rd] $resh
        } else {
#            set ::x[reg2i $rd] "0x[string range $res 18 33]"
            set ::x[reg2i $rd] $res
        }
    }
	proc i2xn {i {n 16}} { ;#i-int_string (signed or unsigned) to hexadecimal_string with n hex digits
		if {$i<0} {
			set i [expr (1<<$n*4)+$i] ;#2's complement
		}
		set ret [format "0x%0${n}llx" $i] ;#ll-verylong integers (no length limit but they have no binary length so the sign is in front of the hex number)
	}
    proc i2xno {i} { ;#integer_string (signed or unsigned) to hexadecimal_string
#        set ret [format 0x%016lx $i]
		set h [expr $i>>64]
		set l [expr $i&0xffffffffffffffff]
        set ret "[format 0x%016lx $h][format %016lx $l]"
    }
		
    proc MUL    {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  * }
    proc MULH   {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  * 0 0 1}
    proc MULHSU {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  * 0 1 1}
    proc MULHU  {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  * 1 1 1}
    proc DIV    {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  / }
    proc DIVU   {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  / 1 1}
    proc REM    {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  % }
    proc REMU   {rd  rs1 rs2}   {typeRM $rd $rs1 $rs2  % 1 1}
    
    proc typeFR  {rd  rs1 rs2 op}   {
#puts "typeFRb: rd=$rd"    
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
#puts "typeFR1: rd=$rd"    
        set res [expr $rs1f $op $rs2f] ;#can't use non-numeric floating-point value as operand of "-" for -NaN(7ffffffffffff)
#puts "typeFR2: rd=$rd"    
#puts "typeFR: rs1f=$rs1f  rs2f=$rs2f res=$res"    
#        set ::f[reg2i $rd] [f2x $res]
		if {[string index $rd 0]!="f"} {
            set ::[string index $rd 0][reg2i $rd] 0x[format "%016lX" $res] ;#rd is int
		} else {
			set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
		}
#puts "typeFRe: rd=$rd"    
    }
    proc FADD.D   {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  + }
    proc FSUB.D   {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  - }
    proc FMUL.D   {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  * }
    proc FDIV.D   {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  / }
    
    proc FEQ.D    {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  == } ;#rd is int
    proc FLT.D    {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  <  } ;#rd is int
    proc FLE.D    {rd  rs1 rs2}   {typeFR $rd $rs1 $rs2  <= } ;#rd is int
    proc FMIN.D  {rd  rs1 rs2}   { ;#fd=min(fs1,fs2)
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
        set res [expr min($rs1f,$rs2f)]
		set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
    }
    proc FMAX.D  {rd  rs1 rs2}   { ;#fd=max(fs1,fs2)
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
        set res [expr max($rs1f,$rs2f)]
		set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
    }
    proc FSGNJ.D  {rd  rs1 rs2}   { ;#sgn(fd)=sgn(fs1)
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
		set res [expr abs($rs2f)]
		if {$rs1f<0} { set res [expr -abs($res)] }
		set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
    }
    proc FSGNJN.D  {rd  rs1 rs2}   { ;#sgn(fd)=-sgn(fs1)
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
		set res [expr abs($rs2f)]
		if {$rs1f>0} { set res [expr -abs($res)] }
		set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
    }
    proc FSGNJX.D  {rd  rs1 rs2}   { ;#sgn(fd)=sgn(fs1) XOR sgn(fs2) +=++,-- (0=00,11); -=+-,-+(1=01,10)
        set rs1f [x2f [set ::f[reg2i $rs1]]]
        set rs2f [x2f [set ::f[reg2i $rs2]]]
		set res [expr abs($rs2f)]
		if {($rs1f>0 && $rs2f>0) || ($rs1f<0 && $rs2f<0)} { set res [expr abs($res)] } else { set res [expr -abs($res)] }
		set ::[string index $rd 0][reg2i $rd] [f2x $res] ;#rd is float
    }



    proc FCVT.L.D    {rd  rs1 rs2}   { ;#Convert FPdouble to signed int
#puts "FCVT.L.D:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1f [x2f [ set ::[string index $rs1 0][reg2i $rs1] ] ] 
        set rs1i [expr round($rs1f)]
#puts "FCVT.L.D:round=[expr round($rs1f)] wide=[expr wide($rs1f)]"    
        if {$rs1i<-pow(2,63) || $rs1i>pow(2,63)-1} {error "ERROR: value out of range"}
        set ::[string index $rd 0][reg2i $rd] [i2x $rs1i]
    }
    proc FCVT.LU.D   {rd  rs1 rs2}   { ;#Convert FPdouble to unsigned int
#puts "FCVT.LU.D:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1f [x2f [ set ::[string index $rs1 0][reg2i $rs1] ] ] 
        set rs1i [expr round($rs1f)]
#puts "FCVT.LU.D:round=[expr round($rs1f)] wide=[expr wide($rs1f)]"    
        if {$rs1i<0 || $rs1i>pow(2,64)-1} {error "ERROR: value out of range"}
        set ::[string index $rd 0][reg2i $rd] [i2x $rs1i]
    }
    proc FCVT.D.L    {rd  rs1 rs2}   { ;#Convert signed int to FPdouble
#puts "FCVT.D.L:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1i [expr [format %ld [set ::[string index $rs1 0][reg2i $rs1]] ] ] 
#puts "FCVT.D.L: rs1i=$rs1i"    
        set ::[string index $rd 0][reg2i $rd] [f2x $rs1i]
    }
    proc FCVT.D.LU   {rd  rs1 rs2}   { ;#Convert unsigned int to FPdouble
#puts "FCVT.D.LU:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1u [x2i [set ::[string index $rs1 0][reg2i $rs1]]] 
#puts "FCVT.D.LU: rs1u=$rs1u"    
        set ::[string index $rd 0][reg2i $rd] [f2x $rs1u]
    }

    proc FMV.X.D   {rd  rs1 rs2}   { ;#Move FPdouble to unsigned int
#puts "FMV.X.D:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1i [ set ::[string index $rs1 0][reg2i $rs1] ]
#puts "FMV.X.D: rs1i=$rs1i"    
        set ::[string index $rd 0][reg2i $rd] [i2x $rs1i]
    }
    proc FMV.D.X   {rd  rs1 rs2}   { ;#Move unsigned int to FPdouble
#puts "FMV.D.X:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1i [x2i [ set ::[string index $rs1 0][reg2i $rs1] ] ] 
#puts "FMV.D.X: rs1i=$rs1i"    
        set ::[string index $rd 0][reg2i $rd] [i2x $rs1i]
    }
    proc FSQRT.D   {rd  rs1 rs2}   { ;#Squareroot
#puts "SQRT.D:rd=$rd rs1=$rs1 rs2=$rs2"
        set rs1f [x2f [ set ::[string index $rs1 0][reg2i $rs1] ] ] 
#puts "SQRT.D.X: rs1f=$rs1f"    
        set ::[string index $rd 0][reg2i $rd] [f2x [expr sqrt($rs1f)]]
    }


    if {0} { 
        ADDI x1 x0 0xfff   
        FCVT.L.D f0 x1   
        FCVT.D.L x2 f0 
          
        ADDI x3 x0 0xfff   
        FCVT.LU.D f1 x3   
        FCVT.D.LU x4 f1   
    }
    
    proc FLmem  {rd rs1 imm nbytes}  {
#  puts "FLmem: rd=$rd rs1=$rs1 imm=$imm nbytes=$nbytes"    
        set a [a64 [set ::x[reg2i $rs1]]+$imm]
        set w "0x[memr $a $nbytes]" ;#0 if never written (silent, no error)
#  puts "FLmem: a=$a w=$w"    
        set ::f[reg2i $rd] [a64 $w]
    }
    proc FLD      {rd  rs1 rs2}   {FLmem $rd  $rs1 $rs2 8}
#   FLD f10 x0 8  
  
    proc FSmem  {rs1 rs2 imm nbytes }  {
        set a [a64 [set ::x[reg2i $rs2]]+$imm]
        set val [string range [set ::f[reg2i $rs1]] [expr 17-$nbytes*2+1] 17]
        memw $a $val
#  puts "FSmem: a=$a val=$val"    
    }
    proc FSD      {rd  rs1 rs2}   {FSmem $rd  $rs1 $rs2 8}
#   FSD f10 x0 0x100  
       
#IMPORTANT HINTS
#p.114 12-bit immediate is sign extended unless it is a ...u instruction
#registers stored as 0x..... but they are always treated as unsigned (large positive)
#"format %ld 0x..." converts 0x.. to signed or "expr wide(0x...)"
;#-1 stays -1 (-1/2=-0.5 rounded to the negative -> -1)
        
    if {0} {
        ADDI x1 x0 0xf
        SD   x1 x0 0
        LD   x2 x0 0
        ANDI x1 x1 0xaa
        ORI  x1 x1 0xaa
        SLLI x2 x1 4
        ADDI x3 x0 -1

        #signed immediate
        ADDI x1 x0 -2
        ADDI x3 x1 1

        #signed immediate SET
        ADDI x1 x0 -1
        ADDI x2 x0 2
        #signed immediate ADD
        ADDI x3 x1 -1 ;#-2
        #signed ADD
        ADD  x3 x1 x2 ;#1
        #signed immediate OR
        ORI x1 x0 -1
        ORI x2 x0 2
        #signed OR
        OR  x3 x1 x2
        #signed immediate AND
        ORI  x1 x0 -1
        ANDI x2 x1 0xf0000000f0000000
        #signed AND
        AND  x3 x1 x2
        #SLL
        ORI  x1 x0 1
        SLLI  x2 x1 4
        ORI  x1 x0 -1
        SLLI  x2 x1 4
        ORI  x1 x0 -1
        SLLI  x2 x1 63
        #SRL
        ORI  x1 x0 16
        SRLI  x2 x1 4
        ORI  x1 x0 -16
        SRLI  x2 x1 4
        ORI  x1 x0 -1
        SRLI  x2 x1 63
        #SRAI
        ORI  x1 x0 16
        SRAI  x2 x1 4
        ORI  x1 x0 -16
        SRAI  x2 x1 3
        ORI  x1 x0 -1
        SRAI  x2 x1 63 ;#-1 stays -1 (-1/2=-0.5 rounded to the negative -> -1)
        #SRAI
        ORI  x1 x0 -16
        ORI  x2 x0 3
        SRA  x3 x1 x2
        #SLTI
        ADDI x1 x0 1
        SLTI x2 x1 2
        SLTI x2 x1 1
        SLTI x2 x1 0
        SLTI x2 x1 -1
        SLTIU x2 x1 -1
        #SLT
        ADDI x3 x0 -1
        SLT x2 x1 x3
        SLTU x2 x1 x3
        #BEQ
        set PC 0
        ADDI x1 x0 1
        BEQ  x1 x0 16
        BNE  x1 x0 16
        ADDI x1 x0 0
        BEQ  x1 x0 16
        BNE  x1 x0 16
        set PC 0
        ADDI x1 x0 1
        BGE  x1 x0 16
        BLT  x1 x0 16
        ADDI x1 x0 -1
        BGE  x1 x0 16
        BLT  x1 x0 16
        BGEU  x1 x0 16
        BLTU  x1 x0 16

        set PC 10
        JAL  x1 2
        ADDI x2 x0 2
        JALR x1 x2 2
        ADDI x2 x0 -2
        JALR x1 x2 2
    }
    
#must be at the end (after their definitions)
regc
catch CompileCmd
} ;#kkcmd end
#
}

#############################################################################
## Initialization Procedure:  init

proc ::init {argc argv} {
puts "init-before widgets"
#console show
}

init $argc $argv

#################################
# VTCL GENERATED GUI PROCEDURES
#

proc vTclWindow. {base} {
    if {$base == ""} {
        set base .
    }
    ###################
    # CREATING WIDGETS
    ###################
    wm focusmodel $top passive
    wm geometry $top 200x200+417+-1248; update
    wm maxsize $top 1989 2335
    wm minsize $top 148 1
    wm overrideredirect $top 0
    wm resizable $top 1 1
    wm withdraw $top
    wm title $top "vtcl-kk01"
    bindtags $top "$top Vtcl-kk01 all"
    vTcl:FireEvent $top <<Create>>
    wm protocol $top WM_DELETE_WINDOW "vTcl:FireEvent $top <<DeleteWindow>>"

    ###################
    # SETTING GEOMETRY
    ###################

    vTcl:FireEvent $base <<Ready>>
}

proc vTclWindow.top44 {base} {
    if {$base == ""} {
        set base .top44
    }
    if {[winfo exists $base]} {
        wm deiconify $base; return
    }
    set top $base
    ###################
    # CREATING WIDGETS
    ###################
    vTcl:toplevel $top -class Toplevel \
		-background {#f0f0f0f0f0f0} -padx 1 -pady 1 
    wm focusmodel $top passive
    wm geometry $top 1234x600+3+3; update
    wm maxsize $top 5000 5000
    wm minsize $top 1234 600
    wm overrideredirect $top 0
    wm resizable $top 1 1
    wm deiconify $top
    wm title $top "RVS (RISC-V Visual Simulator) v0.45"
    vTcl:DefineAlias "$top" "Toplevel1" vTcl:Toplevel:WidgetProc "" 1
    bindtags $top "$top Toplevel all _TopLevel"
    vTcl:FireEvent $top <<Create>>
    wm protocol $top WM_DELETE_WINDOW "vTcl:FireEvent $top <<DeleteWindow>>"

    frame $top.cpd69 \
		-borderwidth 1 -relief sunken -height 25 -width 225 
    vTcl:DefineAlias "$top.cpd69" "Frame2" vTcl:WidgetProc "Toplevel1" 1
    set site_3_0 $top.cpd69
    menubutton $site_3_0.01 \
		-anchor w -menu "$site_3_0.01.02" -padx 4 -pady 3 -text File -width 4 
    vTcl:DefineAlias "$site_3_0.01" "Menubutton1" vTcl:WidgetProc "Toplevel1" 1
    menu $site_3_0.01.02 \
		-activeborderwidth 1 -borderwidth 1 -font {Tahoma 8} -tearoff 0 
    vTcl:DefineAlias "$site_3_0.01.02" "Menu1" vTcl:WidgetProc "" 1
    $site_3_0.01.02 add command \
		-command Load -label Load 
    $site_3_0.01.02 add command \
		-command Append -label Append 
    $site_3_0.01.02 add separator \
		
    $site_3_0.01.02 add command \
		-command Save -label Save 
    $site_3_0.01.02 add command \
		-command SaveAs -label SaveAs 
    $site_3_0.01.02 add separator \
		
    $site_3_0.01.02 add command \
		-command Batch -label Batch 
    $site_3_0.01.02 add separator \
		
    $site_3_0.01.02 add command \
		-command Exit -label Exit 
    menubutton $site_3_0.05 \
		-anchor w -menu "$site_3_0.05.06" -padx 4 -pady 3 -text Help -width 4 
    vTcl:DefineAlias "$site_3_0.05" "Menubutton3" vTcl:WidgetProc "Toplevel1" 1
    menu $site_3_0.05.06 \
		-activeborderwidth 1 -borderwidth 1 -font {Tahoma 8} -tearoff 0 
    vTcl:DefineAlias "$site_3_0.05.06" "Menu1" vTcl:WidgetProc "" 1
    $site_3_0.05.06 add command \
		-command About -label About 
    frame $site_3_0.cpd56 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_3_0.cpd56" "Frame14" vTcl:WidgetProc "Toplevel1" 1
    set site_4_0 $site_3_0.cpd56
    label $site_4_0.lab59 \
		-text Fname: 
    vTcl:DefineAlias "$site_4_0.lab59" "LabelFname" vTcl:WidgetProc "Toplevel1" 1
    label $site_4_0.cpd58 \
		-anchor e -justify right -textvariable ::Fname 
    vTcl:DefineAlias "$site_4_0.cpd58" "LabelFnameVar" vTcl:WidgetProc "Toplevel1" 1
    pack $site_4_0.lab59 \
		-in $site_4_0 -anchor center -expand 0 -fill none -side left 
    pack $site_4_0.cpd58 \
		-in $site_4_0 -anchor center -expand 0 -fill none -side left 
    pack $site_3_0.01 \
		-in $site_3_0 -anchor center -expand 0 -fill none -side left 
    pack $site_3_0.05 \
		-in $site_3_0 -anchor center -expand 0 -fill none -side right 
    pack $site_3_0.cpd56 \
		-in $site_3_0 -anchor w -expand 1 -fill both -side top 
    frame $top.fra76 \
		-borderwidth 2 -relief groove -height 221 -width 125 
    vTcl:DefineAlias "$top.fra76" "Frame1" vTcl:WidgetProc "Toplevel1" 1
    set site_3_0 $top.fra76
    frame $site_3_0.cpd70 \
		-background {#000000} -height 100 -width 200 
    vTcl:DefineAlias "$site_3_0.cpd70" "Frame8" vTcl:WidgetProc "Toplevel1" 1
    set site_4_0 $site_3_0.cpd70
    frame $site_4_0.01 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_4_0.01" "Frame5" vTcl:WidgetProc "Toplevel1" 1
    set site_5_0 $site_4_0.01
    frame $site_5_0.cpd65 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_5_0.cpd65" "Frame47" vTcl:WidgetProc "Toplevel1" 1
    set site_6_0 $site_5_0.cpd65
    frame $site_6_0.cpd57 \
		-background {#000000} -height 100 -width 200 
    vTcl:DefineAlias "$site_6_0.cpd57" "Frame15" vTcl:WidgetProc "Toplevel1" 1
    set site_7_0 $site_6_0.cpd57
    frame $site_7_0.01 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_7_0.01" "Frame56" vTcl:WidgetProc "Toplevel1" 1
    set site_8_0 $site_7_0.01
    frame $site_8_0.cpd57 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_8_0.cpd57" "Frame60" vTcl:WidgetProc "Toplevel1" 1
    set site_9_0 $site_8_0.cpd57
    frame $site_9_0.cpd56 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_9_0.cpd56" "Frame48" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.cpd56
    scrollbar $site_10_0.cpd59 \
		-command "$site_10_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_10_0.cpd59" "Scrollbar22" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_10_0.cpd84 \
		-command "$site_10_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_10_0.cpd84" "Scrollbar23" vTcl:WidgetProc "Toplevel1" 1
    text $site_10_0.cpd85 \
		-background white -height 1 -takefocus 0 -undo 1 -wrap none \
		-xscrollcommand "$site_10_0.cpd84 set" \
		-yscrollcommand "$site_10_0.cpd59 set" 
    vTcl:DefineAlias "$site_10_0.cpd85" "TextSource" vTcl:WidgetProc "Toplevel1" 1
    bind $site_10_0.cpd85 <<Modified>> {
        TextSourceChanged
    }
    pack $site_10_0.cpd59 \
		-in $site_10_0 -anchor center -expand 0 -fill y -side right 
    pack $site_10_0.cpd84 \
		-in $site_10_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_10_0.cpd85 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_9_0.cpd93 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_9_0.cpd93" "Frame63" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.cpd93
    button $site_10_0.cpd76 \
		-command CompileCmd -pady 0 -takefocus 0 -text Compile 
    vTcl:DefineAlias "$site_10_0.cpd76" "ButtonCompile" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_10_0.cpd60 \
		-command CheckbuttonBINCmd -justify right -overrelief flat \
		-takefocus 0 -text BIN -variable CheckbuttonBINVar 
    vTcl:DefineAlias "$site_10_0.cpd60" "CheckbuttonBIN" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_10_0.cpd57 \
		-command CheckbuttonHEXCompCmd -justify right -overrelief flat \
		-takefocus 0 -text HEX -variable CheckbuttonHEXCompVar 
    vTcl:DefineAlias "$site_10_0.cpd57" "CheckbuttonHEXComp" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_10_0.cpd58 \
		-command CheckbuttonINTCompCmd -justify right -overrelief flat \
		-takefocus 0 -text INT -variable CheckbuttonINTCompVar 
    vTcl:DefineAlias "$site_10_0.cpd58" "CheckbuttonINTComp" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_10_0.cpd59 \
		-command CheckbuttonTXTCompCmd -justify right -overrelief flat \
		-takefocus 0 -text TXT -variable CheckbuttonTXTCompVar 
    vTcl:DefineAlias "$site_10_0.cpd59" "CheckbuttonTXTComp" vTcl:WidgetProc "Toplevel1" 1
    label $site_10_0.cpd73 \
		-text Source 
    vTcl:DefineAlias "$site_10_0.cpd73" "Label12" vTcl:WidgetProc "Toplevel1" 1
    pack $site_10_0.cpd76 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd60 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd57 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd58 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd59 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd73 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side right 
    pack $site_9_0.cpd56 \
		-in $site_9_0 -anchor center -expand 1 -fill both -side top 
    pack $site_9_0.cpd93 \
		-in $site_9_0 -anchor center -expand 1 -fill x -side left 
    pack $site_8_0.cpd57 \
		-in $site_8_0 -anchor center -expand 1 -fill both -side top 
    frame $site_7_0.02 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_7_0.02" "Frame64" vTcl:WidgetProc "Toplevel1" 1
    set site_8_0 $site_7_0.02
    frame $site_8_0.cpd100 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_8_0.cpd100" "Frame65" vTcl:WidgetProc "Toplevel1" 1
    set site_9_0 $site_8_0.cpd100
    frame $site_9_0.cpd57 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_9_0.cpd57" "Frame49" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.cpd57
    scrollbar $site_10_0.cpd59 \
		-command "$site_10_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_10_0.cpd59" "Scrollbar24" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_10_0.cpd84 \
		-command "$site_10_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_10_0.cpd84" "Scrollbar25" vTcl:WidgetProc "Toplevel1" 1
    text $site_10_0.cpd85 \
		-background white -height 1 -takefocus 0 -wrap none \
		-xscrollcommand "$site_10_0.cpd84 set" \
		-yscrollcommand "$site_10_0.cpd59 set" 
    vTcl:DefineAlias "$site_10_0.cpd85" "TextListing" vTcl:WidgetProc "Toplevel1" 1
    pack $site_10_0.cpd59 \
		-in $site_10_0 -anchor center -expand 0 -fill y -side right 
    pack $site_10_0.cpd84 \
		-in $site_10_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_10_0.cpd85 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_9_0.cpd102 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_9_0.cpd102" "Frame66" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.cpd102
    button $site_10_0.cpd56 \
		-command ResetPCCmd -pady 0 -takefocus 0 -text InitPC 
    vTcl:DefineAlias "$site_10_0.cpd56" "ButtonResetPC" vTcl:WidgetProc "Toplevel1" 1
    entry $site_10_0.cpd62 \
		-background white -takefocus 0 -textvariable PC 
    vTcl:DefineAlias "$site_10_0.cpd62" "EntryPC" vTcl:WidgetProc "Toplevel1" 1
    button $site_10_0.cpd57 \
		-command {RunCmd 1} -pady 0 -takefocus 0 -text START 
    vTcl:DefineAlias "$site_10_0.cpd57" "ButtonSTART" vTcl:WidgetProc "Toplevel1" 1
    button $site_10_0.cpd60 \
		-command StopCmd -pady 0 -takefocus 0 -text {  Stop  } 
    vTcl:DefineAlias "$site_10_0.cpd60" "ButtonStop" vTcl:WidgetProc "Toplevel1" 1
    button $site_10_0.cpd59 \
		-command RunCmd -pady 0 -takefocus 0 -text {  Run   } 
    vTcl:DefineAlias "$site_10_0.cpd59" "ButtonRun" vTcl:WidgetProc "Toplevel1" 1
    button $site_10_0.cpd58 \
		-command StepCmd -pady 0 -takefocus 0 -text {  Next  } 
    vTcl:DefineAlias "$site_10_0.cpd58" "ButtonStep" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_10_0.cpd63 \
		-command CheckbuttonTXTExeCmd -justify right -overrelief flat \
		-takefocus 0 -text TXT -variable CheckbuttonTXTExeVar 
    vTcl:DefineAlias "$site_10_0.cpd63" "CheckbuttonTXTExe" vTcl:WidgetProc "Toplevel1" 1
    label $site_10_0.cpd74 \
		-text Listing 
    vTcl:DefineAlias "$site_10_0.cpd74" "Label13" vTcl:WidgetProc "Toplevel1" 1
    pack $site_10_0.cpd56 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd62 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd57 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd60 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd59 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd58 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd63 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side left 
    pack $site_10_0.cpd74 \
		-in $site_10_0 -anchor center -expand 0 -fill none -side right 
    pack $site_9_0.cpd57 \
		-in $site_9_0 -anchor center -expand 1 -fill both -side top 
    pack $site_9_0.cpd102 \
		-in $site_9_0 -anchor center -expand 1 -fill x -side left 
    pack $site_8_0.cpd100 \
		-in $site_8_0 -anchor center -expand 1 -fill both -side top 
    frame $site_7_0.03 \
		-borderwidth 2 -relief raised -background {#ff0000} 
    vTcl:DefineAlias "$site_7_0.03" "Frame67" vTcl:WidgetProc "Toplevel1" 1
    bind $site_7_0.03 <B1-Motion> {
        set root [ split %W . ]
	    set nb [ llength $root ]
	    incr nb -1
	    set root [ lreplace $root $nb $nb ]
	    set root [ join $root . ]
	    set width [ winfo width $root ].0
	    
	    set val [ expr (%X - [winfo rootx $root]) /$width ]

	    if { $val >= 0 && $val <= 1.0 } {
	    
		place $root.01 -relwidth $val
		place $root.03 -relx $val
		place $root.02 -relwidth [ expr 1.0 - $val ]
	    }
    }
    place $site_7_0.01 \
		-in $site_7_0 -x 0 -y 0 -width -1 -relwidth 0.4052 -relheight 1 \
		-anchor nw -bordermode ignore 
    place $site_7_0.02 \
		-in $site_7_0 -x 0 -relx 1 -y 0 -width -1 -relwidth 0.5948 \
		-relheight 1 -anchor ne -bordermode ignore 
    place $site_7_0.03 \
		-in $site_7_0 -x 0 -relx 0.4052 -y 0 -rely 0.9 -width 10 -height 10 \
		-anchor s -bordermode ignore 
    pack $site_6_0.cpd57 \
		-in $site_6_0 -anchor center -expand 1 -fill both -side top 
    pack $site_5_0.cpd65 \
		-in $site_5_0 -anchor center -expand 1 -fill both -side top 
    frame $site_4_0.02 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_4_0.02" "Frame6" vTcl:WidgetProc "Toplevel1" 1
    set site_5_0 $site_4_0.02
    frame $site_5_0.cpd74 \
		-background {#000000} -height 100 -width 200 
    vTcl:DefineAlias "$site_5_0.cpd74" "Frame17" vTcl:WidgetProc "Toplevel1" 1
    set site_6_0 $site_5_0.cpd74
    frame $site_6_0.01 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_6_0.01" "Frame18" vTcl:WidgetProc "Toplevel1" 1
    set site_7_0 $site_6_0.01
    frame $site_7_0.cpd69 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_7_0.cpd69" "Frame31" vTcl:WidgetProc "Toplevel1" 1
    set site_8_0 $site_7_0.cpd69
    frame $site_8_0.cpd72 \
		-background {#000000} -height 100 -width 200 
    vTcl:DefineAlias "$site_8_0.cpd72" "Frame32" vTcl:WidgetProc "Toplevel1" 1
    set site_9_0 $site_8_0.cpd72
    frame $site_9_0.01 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_9_0.01" "Frame33" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.01
    frame $site_10_0.cpd57 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_10_0.cpd57" "Frame34" vTcl:WidgetProc "Toplevel1" 1
    set site_11_0 $site_10_0.cpd57
    frame $site_11_0.fra77 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_11_0.fra77" "Frame35" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.fra77
    scrollbar $site_12_0.cpd59 \
		-command "$site_12_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd59" "Scrollbar14" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_12_0.cpd84 \
		-command "$site_12_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd84" "Scrollbar15" vTcl:WidgetProc "Toplevel1" 1
    text $site_12_0.cpd85 \
		-background white -height 1 -state disabled -takefocus 0 -width 40 \
		-wrap none -xscrollcommand "$site_12_0.cpd84 set" \
		-yscrollcommand "$site_12_0.cpd59 set" 
    vTcl:DefineAlias "$site_12_0.cpd85" "TextRegs" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd59 \
		-in $site_12_0 -anchor center -expand 0 -fill y -side right 
    pack $site_12_0.cpd84 \
		-in $site_12_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_12_0.cpd85 \
		-in $site_12_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_11_0.cpd93 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_11_0.cpd93" "Frame36" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd93
    button $site_12_0.cpd60 \
		-command TextRegsCmd -pady 0 -takefocus 0 -text Refresh 
    vTcl:DefineAlias "$site_12_0.cpd60" "ButtonRefreshRegs" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd56 \
		-command CheckbuttonHEXRegCmd -justify right -overrelief flat \
		-takefocus 0 -text HEX -variable CheckbuttonHEXRegVar 
    vTcl:DefineAlias "$site_12_0.cpd56" "CheckbuttonHEXReg" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd57 \
		-command CheckbuttonINTRegCmd -justify right -overrelief flat \
		-takefocus 0 -text INT -variable CheckbuttonINTRegVar 
    vTcl:DefineAlias "$site_12_0.cpd57" "CheckbuttonINTReg" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd58 \
		-command CheckbuttonFLPRegCmd -justify right -overrelief flat \
		-takefocus 0 -text FLP -variable CheckbuttonFLPRegVar 
    vTcl:DefineAlias "$site_12_0.cpd58" "CheckbuttonFLPReg" vTcl:WidgetProc "Toplevel1" 1
    label $site_12_0.cpd75 \
		-text Regs 
    vTcl:DefineAlias "$site_12_0.cpd75" "Label5" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd60 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd56 \
		-in $site_12_0 -anchor e -expand 0 -fill x -side left 
    pack $site_12_0.cpd57 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd58 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd75 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side right 
    pack $site_11_0.fra77 \
		-in $site_11_0 -anchor center -expand 1 -fill both -side top 
    pack $site_11_0.cpd93 \
		-in $site_11_0 -anchor center -expand 1 -fill both -side left 
    pack $site_10_0.cpd57 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side top 
    frame $site_9_0.02 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_9_0.02" "Frame37" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.02
    frame $site_10_0.cpd100 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_10_0.cpd100" "Frame38" vTcl:WidgetProc "Toplevel1" 1
    set site_11_0 $site_10_0.cpd100
    frame $site_11_0.cpd68 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_11_0.cpd68" "Frame39" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd68
    scrollbar $site_12_0.cpd59 \
		-command "$site_12_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd59" "Scrollbar16" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_12_0.cpd84 \
		-command "$site_12_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd84" "Scrollbar17" vTcl:WidgetProc "Toplevel1" 1
    text $site_12_0.cpd85 \
		-background white -height 1 -state disabled -takefocus 0 -wrap none \
		-xscrollcommand "$site_12_0.cpd84 set" \
		-yscrollcommand "$site_12_0.cpd59 set" 
    vTcl:DefineAlias "$site_12_0.cpd85" "TextMemory" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd59 \
		-in $site_12_0 -anchor center -expand 0 -fill y -side right 
    pack $site_12_0.cpd84 \
		-in $site_12_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_12_0.cpd85 \
		-in $site_12_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_11_0.cpd102 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_11_0.cpd102" "Frame40" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd102
    button $site_12_0.cpd58 \
		-command TextMemoryCmd -pady 0 -takefocus 0 -text Refresh 
    vTcl:DefineAlias "$site_12_0.cpd58" "ButtonRefreshMemory" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd60 \
		-command CheckbuttonHEXCmd -justify right -overrelief flat \
		-takefocus 0 -text HEX -variable CheckbuttonHEXVar 
    vTcl:DefineAlias "$site_12_0.cpd60" "CheckbuttonHEX" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd57 \
		-command CheckbuttonINTCmd -justify right -overrelief flat \
		-takefocus 0 -text INT -variable CheckbuttonINTVar 
    vTcl:DefineAlias "$site_12_0.cpd57" "CheckbuttonINT" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd59 \
		-command CheckbuttonFLPCmd -justify right -overrelief flat \
		-takefocus 0 -text FLP -variable CheckbuttonFLPVar 
    vTcl:DefineAlias "$site_12_0.cpd59" "CheckbuttonFLP" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_12_0.cpd61 \
		-command CheckbuttonTXTMemCmd -justify right -overrelief flat \
		-takefocus 0 -text TXT -variable CheckbuttonTXTMemVar 
    vTcl:DefineAlias "$site_12_0.cpd61" "CheckbuttonTXTMem" vTcl:WidgetProc "Toplevel1" 1
    label $site_12_0.cpd64 \
		-text Mem 
    vTcl:DefineAlias "$site_12_0.cpd64" "Label2" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd58 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd60 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd57 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd59 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd61 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd64 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side right 
    pack $site_11_0.cpd68 \
		-in $site_11_0 -anchor center -expand 1 -fill both -side top 
    pack $site_11_0.cpd102 \
		-in $site_11_0 -anchor center -expand 1 -fill x -side left 
    pack $site_10_0.cpd100 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side top 
    frame $site_9_0.03 \
		-borderwidth 2 -relief raised -background {#ff0000} 
    vTcl:DefineAlias "$site_9_0.03" "Frame41" vTcl:WidgetProc "Toplevel1" 1
    bind $site_9_0.03 <B1-Motion> {
        set root [ split %W . ]
	    set nb [ llength $root ]
	    incr nb -1
	    set root [ lreplace $root $nb $nb ]
	    set root [ join $root . ]
	    set width [ winfo width $root ].0
	    
	    set val [ expr (%X - [winfo rootx $root]) /$width ]

	    if { $val >= 0 && $val <= 1.0 } {
	    
		place $root.01 -relwidth $val
		place $root.03 -relx $val
		place $root.02 -relwidth [ expr 1.0 - $val ]
	    }
    }
    place $site_9_0.01 \
		-in $site_9_0 -x 0 -y 0 -width -1 -relwidth 0.4549 -relheight 1 \
		-anchor nw -bordermode ignore 
    place $site_9_0.02 \
		-in $site_9_0 -x 0 -relx 1 -y 0 -width -1 -relwidth 0.5451 \
		-relheight 1 -anchor ne -bordermode ignore 
    place $site_9_0.03 \
		-in $site_9_0 -x 0 -relx 0.4549 -y 0 -rely 0.9 -width 10 -height 10 \
		-anchor s -bordermode ignore 
    pack $site_8_0.cpd72 \
		-in $site_8_0 -anchor center -expand 1 -fill both -side top 
    pack $site_7_0.cpd69 \
		-in $site_7_0 -anchor center -expand 1 -fill both -side top 
    frame $site_6_0.02 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_6_0.02" "Frame22" vTcl:WidgetProc "Toplevel1" 1
    set site_7_0 $site_6_0.02
    frame $site_7_0.fra56 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_7_0.fra56" "Frame4" vTcl:WidgetProc "Toplevel1" 1
    set site_8_0 $site_7_0.fra56
    frame $site_8_0.cpd57 \
		-background {#000000} -height 100 -width 200 
    vTcl:DefineAlias "$site_8_0.cpd57" "Frame9" vTcl:WidgetProc "Toplevel1" 1
    set site_9_0 $site_8_0.cpd57
    frame $site_9_0.01 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_9_0.01" "Frame10" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.01
    frame $site_10_0.cpd63 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_10_0.cpd63" "Frame28" vTcl:WidgetProc "Toplevel1" 1
    set site_11_0 $site_10_0.cpd63
    frame $site_11_0.cpd68 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_11_0.cpd68" "Frame29" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd68
    scrollbar $site_12_0.cpd59 \
		-command "$site_12_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd59" "Scrollbar12" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_12_0.cpd84 \
		-command "$site_12_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_12_0.cpd84" "Scrollbar13" vTcl:WidgetProc "Toplevel1" 1
    text $site_12_0.cpd85 \
		-background white -height 1 -state disabled -takefocus 0 -width 20 \
		-wrap none -xscrollcommand "$site_12_0.cpd84 set" \
		-yscrollcommand "$site_12_0.cpd59 set" 
    vTcl:DefineAlias "$site_12_0.cpd85" "TextExecution" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd59 \
		-in $site_12_0 -anchor center -expand 0 -fill y -side right 
    pack $site_12_0.cpd84 \
		-in $site_12_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_12_0.cpd85 \
		-in $site_12_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_11_0.cpd102 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_11_0.cpd102" "Frame30" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd102
    button $site_12_0.cpd61 \
		-command TextExecutionCmd -pady 0 -takefocus 0 -text {  Clear  } 
    vTcl:DefineAlias "$site_12_0.cpd61" "ButtonClearExecution" vTcl:WidgetProc "Toplevel1" 1
    label $site_12_0.cpd76 \
		-text Exec 
    vTcl:DefineAlias "$site_12_0.cpd76" "Label9" vTcl:WidgetProc "Toplevel1" 1
    pack $site_12_0.cpd61 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side left 
    pack $site_12_0.cpd76 \
		-in $site_12_0 -anchor center -expand 0 -fill none -side right 
    pack $site_11_0.cpd68 \
		-in $site_11_0 -anchor center -expand 1 -fill both -side top 
    pack $site_11_0.cpd102 \
		-in $site_11_0 -anchor center -expand 1 -fill x -side left 
    pack $site_10_0.cpd63 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side top 
    frame $site_9_0.02 \
		-background {#9900991B99FE} 
    vTcl:DefineAlias "$site_9_0.02" "Frame11" vTcl:WidgetProc "Toplevel1" 1
    set site_10_0 $site_9_0.02
    frame $site_10_0.cpd62 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_10_0.cpd62" "Frame3" vTcl:WidgetProc "Toplevel1" 1
    set site_11_0 $site_10_0.cpd62
    frame $site_11_0.cpd60 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_11_0.cpd60" "Frame23" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd60
    frame $site_12_0.cpd68 \
		-borderwidth 2 -relief groove -height 75 
    vTcl:DefineAlias "$site_12_0.cpd68" "Frame24" vTcl:WidgetProc "Toplevel1" 1
    set site_13_0 $site_12_0.cpd68
    scrollbar $site_13_0.cpd59 \
		-command "$site_13_0.cpd85 yview" -takefocus 0 
    vTcl:DefineAlias "$site_13_0.cpd59" "Scrollbar10" vTcl:WidgetProc "Toplevel1" 1
    scrollbar $site_13_0.cpd84 \
		-command "$site_13_0.cpd85 xview" -orient horizontal -takefocus 0 
    vTcl:DefineAlias "$site_13_0.cpd84" "Scrollbar11" vTcl:WidgetProc "Toplevel1" 1
    text $site_13_0.cpd85 \
		-background white -height 1 -state disabled -takefocus 0 -wrap none \
		-xscrollcommand "$site_13_0.cpd84 set" \
		-yscrollcommand "$site_13_0.cpd59 set" 
    vTcl:DefineAlias "$site_13_0.cpd85" "TextOutput" vTcl:WidgetProc "Toplevel1" 1
    pack $site_13_0.cpd59 \
		-in $site_13_0 -anchor center -expand 0 -fill y -side right 
    pack $site_13_0.cpd84 \
		-in $site_13_0 -anchor center -expand 0 -fill x -side bottom 
    pack $site_13_0.cpd85 \
		-in $site_13_0 -anchor center -expand 1 -fill both -side bottom 
    frame $site_12_0.cpd102 \
		-borderwidth 2 -relief groove -height 70 -width 125 
    vTcl:DefineAlias "$site_12_0.cpd102" "Frame25" vTcl:WidgetProc "Toplevel1" 1
    set site_13_0 $site_12_0.cpd102
    button $site_13_0.cpd61 \
		-command TextOutputCmd -pady 0 -takefocus 0 -text {  Clear  } 
    vTcl:DefineAlias "$site_13_0.cpd61" "ButtonClearOutput" vTcl:WidgetProc "Toplevel1" 1
    label $site_13_0.cpd47 \
		-text OUT 
    vTcl:DefineAlias "$site_13_0.cpd47" "Label11" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_13_0.che58 \
		-command CheckbuttonOUTCmd -justify right -overrelief flat \
		-state disabled -takefocus 0 -text DMA -variable CheckbuttonOUTVar 
    vTcl:DefineAlias "$site_13_0.che58" "CheckbuttonOUT" vTcl:WidgetProc "Toplevel1" 1
    pack $site_13_0.cpd61 \
		-in $site_13_0 -anchor center -expand 0 -fill none -side left 
    pack $site_13_0.cpd47 \
		-in $site_13_0 -anchor center -expand 0 -fill none -side right 
    pack $site_13_0.che58 \
		-in $site_13_0 -anchor e -expand 0 -fill none -side right 
    pack $site_12_0.cpd68 \
		-in $site_12_0 -anchor center -expand 1 -fill both -side top 
    pack $site_12_0.cpd102 \
		-in $site_12_0 -anchor center -expand 1 -fill x -side left 
    frame $site_11_0.cpd61 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_11_0.cpd61" "Frame43" vTcl:WidgetProc "Toplevel1" 1
    set site_12_0 $site_11_0.cpd61
    frame $site_12_0.cpd102 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_12_0.cpd102" "FrameInput" vTcl:WidgetProc "Toplevel1" 1
    set site_13_0 $site_12_0.cpd102
    frame $site_13_0.fra56 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_13_0.fra56" "Frame13" vTcl:WidgetProc "Toplevel1" 1
    set site_14_0 $site_13_0.fra56
    label $site_14_0.cpd58 \
		-background gray75 -foreground black -textvariable EntryLabelType \
		-width 1 
    vTcl:DefineAlias "$site_14_0.cpd58" "EntryLabel" vTcl:WidgetProc "Toplevel1" 1
    entry $site_14_0.cpd59 \
		-background white -takefocus 0 -textvariable EntryInputVar -width 5 
    vTcl:DefineAlias "$site_14_0.cpd59" "EntryInput" vTcl:WidgetProc "Toplevel1" 1
    bind $site_14_0.cpd59 <Key-Return> {
        EntryInputCmd
    }
    pack $site_14_0.cpd58 \
		-in $site_14_0 -anchor center -expand 0 -fill none -side left 
    pack $site_14_0.cpd59 \
		-in $site_14_0 -anchor center -expand 1 -fill both -side left 
    frame $site_13_0.fra57 \
		-borderwidth 2 -relief groove -height 75 -width 125 
    vTcl:DefineAlias "$site_13_0.fra57" "Frame16" vTcl:WidgetProc "Toplevel1" 1
    set site_14_0 $site_13_0.fra57
    button $site_14_0.cpd62 \
		-command CancelCmd -pady 0 -takefocus 0 -text {Cancel } 
    vTcl:DefineAlias "$site_14_0.cpd62" "ButtonCancel" vTcl:WidgetProc "Toplevel1" 1
    button $site_14_0.cpd61 \
		-command EntryInputCmd -pady 0 -takefocus 0 -text {  Enter  } 
    vTcl:DefineAlias "$site_14_0.cpd61" "ButtonEnter" vTcl:WidgetProc "Toplevel1" 1
    label $site_14_0.cpd45 \
		-text { INP} 
    vTcl:DefineAlias "$site_14_0.cpd45" "Label10" vTcl:WidgetProc "Toplevel1" 1
    checkbutton $site_14_0.cpd60 \
		-command CheckbuttonINPCmd -state disabled -takefocus 0 -text DMA \
		-variable CheckbuttonINPVar 
    vTcl:DefineAlias "$site_14_0.cpd60" "CheckbuttonINP" vTcl:WidgetProc "Toplevel1" 1
    pack $site_14_0.cpd62 \
		-in $site_14_0 -anchor center -expand 0 -fill none -side left 
    pack $site_14_0.cpd61 \
		-in $site_14_0 -anchor center -expand 0 -fill none -side left 
    pack $site_14_0.cpd45 \
		-in $site_14_0 -anchor center -expand 0 -fill none -side right 
    pack $site_14_0.cpd60 \
		-in $site_14_0 -anchor e -expand 0 -fill none -side right 
    pack $site_13_0.fra56 \
		-in $site_13_0 -anchor center -expand 1 -fill both -side top 
    pack $site_13_0.fra57 \
		-in $site_13_0 -anchor center -expand 1 -fill both -side bottom 
    pack $site_12_0.cpd102 \
		-in $site_12_0 -anchor center -expand 1 -fill x -side left 
    pack $site_11_0.cpd60 \
		-in $site_11_0 -anchor center -expand 1 -fill both -side top 
    pack $site_11_0.cpd61 \
		-in $site_11_0 -anchor center -expand 0 -fill both -side top 
    pack $site_10_0.cpd62 \
		-in $site_10_0 -anchor center -expand 1 -fill both -side top 
    frame $site_9_0.03 \
		-borderwidth 2 -relief raised -background {#ff0000} 
    vTcl:DefineAlias "$site_9_0.03" "Frame12" vTcl:WidgetProc "Toplevel1" 1
    bind $site_9_0.03 <B1-Motion> {
        set root [ split %W . ]
	    set nb [ llength $root ]
	    incr nb -1
	    set root [ lreplace $root $nb $nb ]
	    set root [ join $root . ]
	    set width [ winfo width $root ].0
	    
	    set val [ expr (%X - [winfo rootx $root]) /$width ]

	    if { $val >= 0 && $val <= 1.0 } {
	    
		place $root.01 -relwidth $val
		place $root.03 -relx $val
		place $root.02 -relwidth [ expr 1.0 - $val ]
	    }
    }
    place $site_9_0.01 \
		-in $site_9_0 -x 0 -y 0 -width -1 -relwidth 0.5048 -relheight 1 \
		-anchor nw -bordermode ignore 
    place $site_9_0.02 \
		-in $site_9_0 -x 0 -relx 1 -y 0 -width -1 -relwidth 0.4952 \
		-relheight 1 -anchor ne -bordermode ignore 
    place $site_9_0.03 \
		-in $site_9_0 -x 0 -relx 0.5048 -y 0 -rely 0.9 -width 10 -height 10 \
		-anchor s -bordermode ignore 
    pack $site_8_0.cpd57 \
		-in $site_8_0 -anchor center -expand 1 -fill both -side top 
    pack $site_7_0.fra56 \
		-in $site_7_0 -anchor center -expand 1 -fill both -side top 
    frame $site_6_0.03 \
		-borderwidth 2 -relief raised -background {#ff0000} 
    vTcl:DefineAlias "$site_6_0.03" "Frame26" vTcl:WidgetProc "Toplevel1" 1
    bind $site_6_0.03 <B1-Motion> {
        set root [ split %W . ]
	    set nb [ llength $root ]
	    incr nb -1
	    set root [ lreplace $root $nb $nb ]
	    set root [ join $root . ]
	    set width [ winfo width $root ].0
	    
	    set val [ expr (%X - [winfo rootx $root]) /$width ]

	    if { $val >= 0 && $val <= 1.0 } {
	    
		place $root.01 -relwidth $val
		place $root.03 -relx $val
		place $root.02 -relwidth [ expr 1.0 - $val ]
	    }
    }
    place $site_6_0.01 \
		-in $site_6_0 -x 0 -y 0 -width -1 -relwidth 0.5733 -relheight 1 \
		-anchor nw -bordermode ignore 
    place $site_6_0.02 \
		-in $site_6_0 -x 0 -relx 1 -y 0 -width -1 -relwidth 0.4267 \
		-relheight 1 -anchor ne -bordermode ignore 
    place $site_6_0.03 \
		-in $site_6_0 -x 0 -relx 0.5733 -y 0 -rely 0.9 -width 10 -height 10 \
		-anchor s -bordermode ignore 
    pack $site_5_0.cpd74 \
		-in $site_5_0 -anchor center -expand 1 -fill both -side top 
    frame $site_4_0.03 \
		-borderwidth 2 -relief raised -background {#ff0000} 
    vTcl:DefineAlias "$site_4_0.03" "Frame7" vTcl:WidgetProc "Toplevel1" 1
    bind $site_4_0.03 <B1-Motion> {
        set root [ split %W . ]
		set nb [ llength $root ]
		
		incr nb -1
		
		set root [ lreplace $root $nb $nb ]
		set root [ join $root . ]
		set height [ winfo height $root ].0
		    
		set val [ expr (%Y - [winfo rooty $root]) /$height ]

		if { $val >= 0 && $val <= 1.0 } {
		    
			place $root.01 -relheight $val
			place $root.03 -rely $val
			place $root.02 -relheight [ expr 1.0 - $val ]
		}
    }
    place $site_4_0.01 \
		-in $site_4_0 -x 0 -y 0 -relwidth 1 -height -1 -relheight 0.4823 \
		-anchor nw -bordermode ignore 
    place $site_4_0.02 \
		-in $site_4_0 -x 0 -y 0 -rely 1 -relwidth 1 -height -1 \
		-relheight 0.5177 -anchor sw -bordermode ignore 
    place $site_4_0.03 \
		-in $site_4_0 -x 0 -relx 0.9 -y 0 -rely 0.4823 -width 10 -height 10 \
		-anchor e -bordermode ignore 
    pack $site_3_0.cpd70 \
		-in $site_3_0 -anchor center -expand 1 -fill both -side top 
    ###################
    # SETTING GEOMETRY
    ###################
    grid columnconf $top 0 -weight 10
    grid rowconf $top 0 -weight 10
    pack $top.cpd69 \
		-in $top -anchor center -expand 0 -fill x -side top 
    pack $top.fra76 \
		-in $top -anchor center -expand 1 -fill both -side top 

    vTcl:FireEvent $base <<Ready>>
}

#############################################################################
## Binding tag:  _TopLevel

bind "_TopLevel" <<Create>> {
    if {![info exists _topcount]} {set _topcount 0}; incr _topcount
}
bind "_TopLevel" <<DeleteWindow>> {
    if {[set ::%W::_modal]} {
                vTcl:Toplevel:WidgetProc %W endmodal
            } else {
                destroy %W; if {$_topcount == 0} {exit}
            }
}
bind "_TopLevel" <Destroy> {
    if {[winfo toplevel %W] == "%W"} {incr _topcount -1}
}
#############################################################################
## Binding tag:  _vTclBalloon


if {![info exists vTcl(sourcing)]} {
bind "_vTclBalloon" <<KillBalloon>> {
    namespace eval ::vTcl::balloon {
	after cancel $id
	if {[winfo exists .vTcl.balloon]} {
	    destroy .vTcl.balloon
	}
	set set 0
    }
}
bind "_vTclBalloon" <<vTclBalloon>> {
    if {$::vTcl::balloon::first != 1} {break}

    namespace eval ::vTcl::balloon {
	set first 2
	if {![winfo exists .vTcl]} {
	    toplevel .vTcl; wm withdraw .vTcl
	}
	if {![winfo exists .vTcl.balloon]} {
	    toplevel .vTcl.balloon -bg black
	}
	wm overrideredirect .vTcl.balloon 1
	label .vTcl.balloon.l  -text ${%W} -relief flat  -bg #ffffaa -fg black -padx 2 -pady 0 -anchor w
	pack .vTcl.balloon.l -side left -padx 1 -pady 1
	wm geometry  .vTcl.balloon  +[expr {[winfo rootx %W]+[winfo width %W]/2}]+[expr {[winfo rooty %W]+[winfo height %W]+4}]
	set set 1
    }
}
bind "_vTclBalloon" <Button> {
    namespace eval ::vTcl::balloon {
	set first 0
    }
    vTcl:FireEvent %W <<KillBalloon>>
}
bind "_vTclBalloon" <Enter> {
    namespace eval ::vTcl::balloon {
	## self defining balloon?
	if {![info exists %W]} {
	    vTcl:FireEvent %W <<SetBalloon>>
	}
	set set 0
	set first 1
	set id [after 500 {vTcl:FireEvent %W <<vTclBalloon>>}]
    }
}
bind "_vTclBalloon" <Leave> {
    namespace eval ::vTcl::balloon {
	set first 0
    }
    vTcl:FireEvent %W <<KillBalloon>>
}
bind "_vTclBalloon" <Motion> {
    namespace eval ::vTcl::balloon {
	if {!$set} {
	    after cancel $id
	    set id [after 500 {vTcl:FireEvent %W <<vTclBalloon>>}]
	}
    }
}
}

Window show .
Window show .top44

main $argc $argv
