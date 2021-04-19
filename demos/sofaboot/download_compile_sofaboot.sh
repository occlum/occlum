#!/bin/bash
if [ ! -d "sofa-boot-guides" ]; then
    git clone https://github.com/sofastack-guides/sofa-boot-guides.git
fi
cd sofa-boot-guides/sofaboot-sample-standard/
mvn compile
mvn package
