set terminal qt
set title 'Timestamps per Message'
set xlabel 'Messages'
set ylabel 'Timestamps'
plot 'data.tsv' using 1:2 with linespoints linewidth 1.5 linecolor rgb 'blue'
