extends #Normal cc::standard[@applepie]
//this is an example of some code for pyxis, so the example we're doing is a recreation of strlen

var str
var length

defi main():
  while (str[length] !: nil):
    length.add(1)!
  return &length.source

//another example this time showing if dog then woof and if cat then meow

var animal

defi main():
  shift animal:
    "cat":
      print "meow"!
    "dog":
      print "woof"!
 else:
  break?(else if return)!

+ = plus
- = minus
* = multiplication
/ = division
%=* = modulus
%? = 
! = end of line marker
!: = not
:: = equals
: = equal equal
& = 
;; = 
// = comment
? = safe error handle
-> = equals
;-> = specifier type
@ = extra variable
@wrafs = static
