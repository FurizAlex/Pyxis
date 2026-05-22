// --- Test

class Animal:
    speak():
		print("hello")!

class Cat < Animal:

var cat :: Cat()!
print cat!
cat.speak()!

// --- Expected
// Instance of 'Cat'
// "Hello"

