#!/bin/bash
set -e

if [[ $1 == "jdk8" ]]; then
    echo ""
    echo "*** Build sofaboot demo with openjdk 8 ***"
    echo "*** Make sure openjdk 8 is installed ***"
    mvn -v | grep "java-8"
else
    echo ""
    echo "*** Build sofaboot demo with openjdk 11 ***"
    echo "*** Make sure openjdk 11 is installed ***"
    mvn -v | grep "java-11"
fi

if [ ! -d "sofa-boot-guides" ]; then
    git clone https://github.com/sofastack-guides/sofa-boot-guides.git
fi

cd sofa-boot-guides/sofaboot-sample-standard/
mvn clean
mvn compile
mvn package
