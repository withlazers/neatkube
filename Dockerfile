FROM clux/muslrust:1.60.0 AS build

WORKDIR /src

COPY . .

RUN \
	mkdir -p /cargo/cargo && \
	ln -sf $HOME/.cargo/config /cargo/cargo && \
	CARGO_HOME=/cargo/cargo \
	CARGO_TARGET_DIR=/cargo/target \
	cargo install \
		--path . \
		--root /app

FROM alpine:3.16

COPY --from=build /app/bin/nk /usr/local/bin/nk

ENV NK_DATA_DIR=/nk

RUN mkdir /nk && \
	cd  && \
	for i in $(nk toolbox list); do \
		ln -sv nk "/usr/local/bin/$i" || exit 1; \
	done
