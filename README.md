WORKING

These repos are to show my Rust learning speed, the applicability of my non-Rust programming background to Rust, and my knowledge of Rust features:

https://github.com/kermit4/first_2_hours_of_rust     
https://github.com/kermit4/first_8_hours_of_rust      (this repo)
https://github.com/kermit4/first_16_hours_of_rust     

If run with no args, it will listen for uploads.

With args it will send a file.  (It does not exit when it's done, and it may stall as there is no timer yet.)

i.e.
```

cargo build
./target/debug/udp_uploader &
sleep 1
./target/debug/udp_uploader /etc/passwd 127.0.0.1:34254
```

should result in a file called "f000.." with the same content

## DONE:
	* retransmission
	* transmission window scaling
## TODO:
	* recovery from complete stall
	* use real hash
	* use larger than 32 byte blocks (damn serde limit!)
	* tail end of transfer has a lot of dups
	* better variable names
	* reduce code duplication and break up long receive() function
