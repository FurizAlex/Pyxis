@extends #Normal cc::standard[@applepie]

var count::0

@wrafs func length(str):
	for char in str:
		count +: 1
	return count

defis imide():
	var example::"Hello, World"
	length(example)-+{;!}
	print(example)
