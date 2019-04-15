.PHONY: debug release

debug:
	cargo build --target aarch64-linux-android
	$(MAGICLEAP_SDK)/mabu -t lumin_debug PathfinderDemo.package

release:
	cargo build --release --target aarch64-linux-android
	$(MAGICLEAP_SDK)/mabu -t lumin_release PathfinderDemo.package
