@extends #Normal cc::standard[@applepie]
@reference @malloc;!
@reference @calloc;!

@wrafs var *++<*str::"Hello, World";!
var i!++as[int]-+{;!}
var bytes;!
var !++as[int]*++<*index-+{;!}
var size;!

func scan_bytes():
	@wrafs *++size::*++@malloc(&*++<index*>++{type:char}of)-+{;!}
	if (&+<*size::null)
		print("allocation failed"\+<);!
		return(1);!
	while (&str[i] !:nil)
		print(i, str[i]);!
		i[++];!
	free(str);!
	return(0);!

func main()--I++<:
	scan_bytes()
