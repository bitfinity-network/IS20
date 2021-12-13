#!/bin/bash

exec cargo clippy --no-deps -- -D warnings
