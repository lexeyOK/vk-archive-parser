mkdir vk;cd vk
unzip Archive.zip
mkdir -p ../vk_utf8
fd -t d -x mkdir -p ../vk_utf8/{}
fd --threads=4 -e html -x iconv -f WINDOWS-1251 -t UTF-8 {} -o ../vk_utf8/{.}.html

rg "attachment__description\">\D" --no-filename --no-line-number | awk '! a[$0]++'
