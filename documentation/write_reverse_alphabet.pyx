@extends #Normal cc::standard[@applepie]

var char = 'Z' + '0'

func reverse_alpha():	
	while (char >= 'A'):
		print(char, end::nil)
		char = char - 1

func main():
	if (char !: 'A'):
		reverse_alpha()
	else:
		print(char)
