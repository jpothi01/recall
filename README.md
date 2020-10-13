Notes and task tracking for people bad at notes and task tracking.

```
# Remember tasks
recall "Take the dog for a walk"
recall
> 0 2020-10-12 20:19:20		    Take the dog for a walk

# Remember internet stuff
recall "Cool thing on stack overflow, should check it out later" --link https://stackoverflow.com/questions/39785597/how-do-i-get-a-slice-of-a-vect-in-rust
recall
> 0 2020-10-12 20:19:20		    Take the dog for a walk
> 1 2020-10-12 20:16:14	link	Cool thing on stack overflow, should check it out later

recall 1
> https://stackoverflow.com/questions/39785597/how-do-i-get-a-slice-of-a-vect-in-rust
> *opens the link*

# Remember stuff on your filesystem
recall "Checkout these logs" --path ~/Downloads/Logs\ 4
recall
> 0 2020-10-12 20:19:20		    Take the dog for a walk
> 1 2020-10-12 20:16:14	link	Cool thing on stack overflow, should check it out later
> 2 2020-10-12 20:16:14	path	Checkout these logs

recall 2
> /Users/jpothier/Downloads/Logs\ 4
> *opens finder to the path*

# Forget things
recall --archive 0
recall
> 0 2020-10-12 20:16:14	link	Cool thing on stack overflow, should check it out later
> 1 2020-10-12 20:16:14	path	Checkout these logs
```