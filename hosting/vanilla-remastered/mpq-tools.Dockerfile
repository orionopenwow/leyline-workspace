# vr-mpq-tools:1 — StormLib (its authors' public source, pinned tag) + python, for vr-slice.py.
# Built on the server by vr-data.sh (docker build -f mpq-tools.Dockerfile), never pushed anywhere. No game data in it.
FROM debian:bookworm-slim AS build
ARG STORMLIB_TAG=v9.26
RUN apt-get update && apt-get install -y --no-install-recommends build-essential cmake git ca-certificates zlib1g-dev libbz2-dev \
    && rm -rf /var/lib/apt/lists/*
RUN git clone -q --depth 1 --branch "${STORMLIB_TAG}" https://github.com/ladislav-zezula/StormLib /src \
    && cmake -S /src -B /b -DBUILD_SHARED_LIBS=ON -DCMAKE_BUILD_TYPE=Release -DSTORM_SKIP_INSTALL=OFF \
    && cmake --build /b -j"$(nproc)" && cmake --install /b --prefix /opt/storm

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends python3 zlib1g libbz2-1.0 && rm -rf /var/lib/apt/lists/*
COPY --from=build /opt/storm/lib/ /usr/local/lib/
RUN ldconfig && ls /usr/local/lib | grep -i storm
ENV STORMLIB=libstorm.so
LABEL leyline.vr=1 org.opencontainers.image.description="StormLib + python for the Vanilla Remastered slice"
