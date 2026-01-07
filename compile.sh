#!/bin/bash

cmake -B build
cmake --build build
cmake --install build
cargo build --release
