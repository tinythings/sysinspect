#!/usr/bin/bash

PID=$1

t1=$(cat /proc/$PID/stat | awk '{print $14 + $15}')
sleep 1
t2=$(cat /proc/$PID/stat | awk '{print $14 + $15}')

process_cpu_time=$((t2 - t1))

total_cpu_time=$(grep '^cpu ' /proc/stat | awk '{print $2 + $3 + $4 + $5 + $6 + $7 + $8}')
sleep 1
total_cpu_time_2=$(grep '^cpu ' /proc/stat | awk '{print $2 + $3 + $4 + $5 + $6 + $7 + $8}')
total_cpu_time_diff=$((total_cpu_time_2 - total_cpu_time))
cpu_usage=$(echo "scale=2; 100 * $process_cpu_time / $total_cpu_time_diff" | bc)

echo "CPU usage: $cpu_usage"

