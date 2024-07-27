.PHONY: build build-android

build:
	cargo build --release

build-android-bindgen: build
	cargo run -p uniffi-bindgen generate --library target/release/liblocalax.so --language kotlin --out-dir android/app/src/main/java
