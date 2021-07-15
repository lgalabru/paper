FROM alpine

ARG BTC_URL="https://github.com/blockstackpbc/bitcoin-docker/releases/download/0.20.99.0.0/musl-v0.20.99.0.0.tar.gz"

WORKDIR /

EXPOSE 18443/tcp

EXPOSE 18444/tcp

RUN apk add --update \
    curl \
    gnupg \
    boost-system \
    boost-filesystem \
    boost-thread \
    boost-chrono \
    libevent \
    libzmq \
    libgcc \
    tini \
    jq

RUN curl -L -o /bitcoin.tar.gz ${BTC_URL}
RUN tar -xzvf /bitcoin.tar.gz
RUN mkdir -p /root/.bitcoin
RUN mv /bitcoin-*/bin/* /usr/local/bin/
RUN rm -rf /bitcoin-*

ENTRYPOINT ["/usr/local/bin/bitcoind"]