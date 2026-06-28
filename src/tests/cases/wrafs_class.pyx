class Player():
	func init(hp):
		this.hp :: hp!

var p1 :: Player(100)!
@wrafs var p2 :: p1!

p2.hp :: 50!
print(p1.hp)!

p1.hp :: 5!
print(p2.hp)!