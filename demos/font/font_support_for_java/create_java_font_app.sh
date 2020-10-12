#!/bin/bash

SCRIPT_NAME=build_java_font_app.sh
# 1. Create the poi demo with font
demopath=`pwd`
rm -rf poi-excel-demo && mkdir poi-excel-demo && cd $_
mkdir -p src/main/java && cd $_

echo '
import org.apache.poi.xssf.streaming.SXSSFRow;
import org.apache.poi.xssf.streaming.SXSSFSheet;
import org.apache.poi.xssf.streaming.SXSSFWorkbook;
import org.apache.poi.xssf.usermodel.XSSFFont;
import org.apache.poi.ss.usermodel.CellStyle;
import org.apache.poi.xssf.streaming.SXSSFCell;
import java.io.FileOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
public class SXSSFWriteDemoTwo {
    public static void main(String[] args) throws IOException {
        // 数据源
        List<String[]> data = new ArrayList<>();
        data.add(new String[]{"学号", "姓名", "性别", "专业"});
        data.add(new String[]{"1001", "小白", "女", "计算机科学与技术"});
        data.add(new String[]{"1002", "小黑", "男", "软件工程"});
        // 关闭自动转移（内存向硬盘）
        SXSSFWorkbook swb = new SXSSFWorkbook(-1);
        // 创建sheet
        SXSSFSheet sheet = swb.createSheet("Demo");
        // 设置font字体
        XSSFFont font = (XSSFFont) swb.createFont();
        font.setFontName("宋体");
        CellStyle cellFormat = swb.createCellStyle();
        cellFormat.setFont(font);
        for (int i = 0; i < data.size(); i++) {
            if (0 != i && 0 == i % 1000) {
                // 手动转移（内存向硬盘）
                sheet.flushRows(1000);
            }
            // 创建行
            SXSSFRow row = sheet.createRow(i);
            String[] content = data.get(i);
            for (int j = 0; j < content.length; j++) {
                // 创建单元格
                if (0 != i && 0 == j) {
                    SXSSFCell c = row.createCell(j);
                    c.setCellStyle(cellFormat);
                    c.setCellValue(Double.valueOf(content[j]));
                    continue;
                }
                SXSSFCell c = row.createCell(j);
                c.setCellStyle(cellFormat);
                c.setCellValue(content[j]);
            }
        }
        // 生成Excel
        FileOutputStream out = new FileOutputStream("/host/Demo.xlsx");
        swb.write(out);
        out.close();
    }
}' > SXSSFWriteDemoTwo.java

cd $demopath/poi-excel-demo

# 2. Create gradle project
echo "
apply plugin: 'java'
repositories {
    mavenCentral()
}
task customFatJar(type: Jar) {
    manifest {
        attributes 'Main-Class': 'SXSSFWriteDemoTwo'
    }
    baseName = 'SXSSFWriteDemoTwo'
    from { configurations.compile.collect { it.isDirectory() ? it : zipTree(it) } }
    with jar
}
dependencies {
    compile 'org.apache.poi:poi:3.17'
    compile 'org.apache.poi:poi-ooxml:3.17'
}" > build.gradle

cd $demopath

rm -rf SimSun.ttf && wget http://d.xiazaiziti.com/en_fonts/fonts/s/SimSun.ttf && mv SimSun.ttf simsun.ttf

docker images | grep occlum-font &> /dev/null
if [ $? -ne 0 ]
then
    echo 'docker build occlum-font:v1 image'
    docker build -t occlum-font:v1 .
else
    echo 'docker image occlum-font:v1 exist'
fi

docker run -it --rm -v `pwd`:`pwd` -w `pwd` --network host  --entrypoint=/bin/sh occlum-font:v1 `pwd`/$SCRIPT_NAME
