.PHONY: build-android

build-android-bindgen:
	cargo build --release
	cargo run -p uniffi-bindgen generate --library target/release/liblocalax.so --language kotlin --out-dir android/app/src/main/java
