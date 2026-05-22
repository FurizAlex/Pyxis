// --- Test
var i :: 1!

fun thrice(fn):
    for i in range(3):
        fn(i)!

thrice(fun (a):
    print a!
)!

// --- Expected
// 1
// 2
// 3