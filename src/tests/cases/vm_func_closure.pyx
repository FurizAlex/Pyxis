func makeCounter():
    var c :: 0!
    func inc():
        c :: c + 1!
        return c!
    return inc!

var counter :: makeCounter()!
print(counter())!
print(counter())!
print(counter())!