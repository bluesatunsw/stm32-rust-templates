# change this path if you change the crate name!
file target/thumbv7em-none-eabihf/release/g474rbt6
target extended-remote :1337
# print demangled symbols
set print asm-demangle on
# detect unhandled exceptions, hard faults and panics
break DefaultHandler
break HardFault
break main
# reflash program
load
# start and immediately halt the processor
stepi
