# Mede FPCW nos pontos de interesse; continua automaticamente. Rodado por probe.py.
set pagination off
set confirm off
set architecture i386
set $n = 0
define rep
  printf "%s fctrl=%#x tid=%d eip=%#x date=%#x tod=%#x rng=%#x\n", $arg0, $fctrl, $_thread, $eip, *(unsigned int*)0x5704f8, *(unsigned short*)0x5704fc, *(unsigned int*)0x55d2c0
end
break *0x404e30
commands
  silent
  set $n = $n + 1
  if $n % 50 == 1
    rep "GATE"
  end
  continue
end
break *0x4a5cb0
commands
  silent
  rep "SIM_ADVANCE"
  continue
end
break *0x4bf760
commands
  silent
  rep "REVENUE"
  continue
end
