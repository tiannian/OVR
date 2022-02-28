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
	- rm $(shell which tendermint)
	bash tools/download_tendermint.sh 'tools/tendermint'
	cd tools/tendermint && $(MAKE) install

prodenv:
	bash tools/create_prod_env.sh


# CI 
ci_build_binary_rust_base:
	docker build -t binary-rust-base -f container/Dockerfile-binary-rust-base .

ci_build_dev_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-dev .
	
ci_build_release_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-release .

ci_build_image:
	@ if [ -d "./binary" ]; then \
		rm -rf ./binary || true; \
	fi
	@ docker run --rm -d --name ovrd-binary ovrd-binary-image:$(IMAGE_TAG)
	@ docker cp ovrd-binary:/binary ./binary
	@ docker rm -f ovrd-binary
	@ docker build -t $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) -f container/Dockerfile-goleveldb .
ifeq ($(ENV),release)
	docker tag $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

ci_push_image:
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

clean_image:
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif