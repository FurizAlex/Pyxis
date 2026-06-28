var outer :: 1!

func makeAlias():
    @wrafs var inner :: outer!
    inner :: 77!
    return inner!

print(makeAlias())!
print(outer)!