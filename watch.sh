#!/bin/bash

filewatcher -I -r ./src/ test.sh 'printf "\ec" && ./test.sh'