CXX := occlum-g++
CXXFLAGS := -std=c++11

.PHONY: all clean

all: hello_world

hello_world: hello_world.cc
	$(CXX) $(CXXFLAGS) $^ -o $@

clean:
	rm -rf hello_world
