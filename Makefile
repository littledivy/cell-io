dev:
	sudo pkill cargo || true
	sudo cargo +stable watch -x "build --features systems/dynamic" &
	sudo cargo +stable run --features reload
