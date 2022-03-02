all: release

lint:
	cargo clippy
	cargo check --tests
	cargo check --benches
	cargo check --examples

test:
	cargo test -- --test-threads=1
	cargo test --release -- --test-threads=1

bench:
	cargo bench

fmt:
	bash tools/fmt.sh

clean:
	cargo clean
	git stash
	git clean -fdx

update:
	cargo update

doc:
	cargo doc --open

define pack
	- rm -rf $(1)
	- rm ~/.cargo/bin/{ovr,ovrd,tendermint}
	mkdir $(1)
	cp \
		./target/$(2)/$(1)/ovr \
		./target/$(2)/$(1)/ovrd \
		$(shell go env GOPATH)/bin/tendermint \
		$(1)/
	cp $(1)/* ~/.cargo/bin/
	cd $(1)/ && ./ovrd pack
	cp -f /tmp/ovrd $(1)/
	cp -f /tmp/ovrd ~/.cargo/bin/
endef

build: tendermint
	cargo build --bins
	$(call pack,debug)

release: build_release

release_rocksdb: build_release_rocksdb

build_release: tendermint
	cargo build --release --bins
	$(call pack,release)

build_release_rocksdb: tendermint
	cargo build --release --bins --no-default-features --features="vsdb_rocksdb"
	$(call pack,release)

build_release_musl: tendermint
	cargo build --release --bins --target=x86_64-unknown-linux-musl
	$(call pack,release,x86_64-unknown-linux-musl)

tendermint:
	-@ rm $(shell which tendermint)
	bash tools/download_tendermint.sh 'tools/tendermint'
	cd tools/tendermint && $(MAKE) install

prodenv:
	bash tools/create_prod_env.sh
