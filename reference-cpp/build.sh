cd "$(dirname "$0")"
mkdir -p build
g++ -std=c++11 main.cpp -o build/main -O3 -ltbb
