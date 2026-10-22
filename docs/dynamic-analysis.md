# Análise dinâmica: o original no Wine (sandbox + gdb)

Regra: `/home/pedryvo/Games/railroad-tycoon-2` é SOMENTE LEITURA. O jogo grava `rt2.log`, `rt2_tsc.CFG`, `hiscore.dat` e saves na própria pasta, então ele **nunca** roda de lá. Tudo roda numa cópia (sandbox).

## 1. Sandbox e execução

```
re/scripts/wine/run_original.sh <sandbox>            # cria (se faltar) e abre o jogo
re/scripts/wine/run_original.sh <sandbox> --no-run   # só cria a cópia
```
- Copia `RT2_PLAT.EXE.orig` como `RT2_PLAT.EXE` (o EXE original, não os patcheados), DLLs (`ddraw.dll` do cnc-ddraw, `Mss32`, `SMACKW32`), `*.lng`, `data`, `data2`, `maps`, `games`, `presets`, `Shaders`. `anim/` fica vazia e `music` não é copiada: o jogo inicia sem elas (~430 MB no total).
- Gera um `ddraw.ini` próprio: 800x600, janelado, `renderer=gdi` (sem OpenGL/FSR). O da instalação é fullscreen 4K.
- `WINEPREFIX` fica em `<sandbox>/../wineprefix` (no scratchpad da sessão: `.../scratchpad/rt2sandbox` e `.../scratchpad/wineprefix`). Wine 11.20; `ddraw=n,b` para usar o cnc-ddraw.
- O script recusa um sandbox dentro da instalação.
- **Fechar a janela encerra a sessão** (é um exit normal, não crash). Para reiniciar: rode o script de novo (ou `probe.py`). Antes disso, limpe sessões velhas: `pkill -x winedbg.exe; WINEPREFIX=<prefix> wineserver -k`. Cuidado: `pkill -f gdb` pode matar o próprio shell.

## 2. Depurador: o que funciona

`gdb` com `attach` não serve: `ptrace_scope=1` (só filho direto) e o Wine usa vários processos. **Funciona: `winedbg --gdb --no-start` (gdbserver do Wine) + `gdb` com `target remote`.**

```
wine winedbg --gdb --no-start 'Z:\caminho\sandbox\RT2_PLAT.EXE'   # imprime: target remote localhost:PORTA
gdb -q -ex 'set architecture i386' -ex 'target remote localhost:PORTA'
```
O caminho precisa ser absoluto no formato `Z:\...` (nome relativo dá "Couldn't start process"). O processo é 32 bits; sem `set architecture i386` o gdb mostra registradores errados. O gdb para na entrada (`0x51c155`).

Driver pronto: `re/scripts/wine/probe.py <sandbox> [script.gdb] [segundos]` (sobe winedbg, acha a porta, roda o gdb em batch, mata tudo no fim). Script de medida: `re/scripts/wine/fpu_probe.gdb` (breakpoints em `0x404e30`, `0x4a5cb0`, `0x4bf760`, imprime `$fctrl`, thread, data, hora e RNG). Execução típica:

```
re/scripts/wine/run_original.sh $SB --no-run
re/scripts/wine/probe.py $SB re/scripts/wine/fpu_probe.gdb 75 | grep -E '^(GATE|SIM|REV)'
```
Dicas: `break *0x404e30` + `commands ... continue end` (breakpoint por endereço funciona sem símbolos); `info threads`; `info float`; `p/x $fctrl`. Memória (endereços de `docs/spec/time.md`): `x/wx 0x5704f8` data (u16 ano, u8 mês, u8 dia), `x/hx 0x5704fc` hora/minuto, `x/wx 0x5704f4` minutos absolutos, `x/wx 0x55d2c0` estado do RNG. Ex.: `date=0x1403074e` = ano 0x74e (1870), mês 3, dia 0x14.

## 3. Medição da palavra de controle da FPU

Evidência (`probe.py`, 75 s, jogo no menu inicial, sem tocar na janela; saídas em `fpu_probe.gdb`):

```
initial fctrl=0x27f                      (parada na entrada 0x51c155, thread principal)
Thread 1 hit Temporary breakpoint 1, 0x00404e30   -> fctrl=0x27f
SIM_ADVANCE fctrl=0x27f tid=1 eip=0x4a5cb0 date=0x1403074e tod=0xf02 rng=0xcad3f502
GATE        fctrl=0x27f tid=1 eip=0x404e30 ...
```
Contagem: 187 amostragens do porteiro `0x404e30` (1 a cada 50 chamadas, ou seja ~9300 chamadas) e 236 chamadas de `sim_advance 0x4a5cb0`, **todas com `$fctrl=0x27f` e na thread 1** (a principal). A amostragem cobre o período depois que o Miles/som já iniciou (as threads extras do Mss32/Smacker aparecem logo no começo: `New Thread` x7, uma delas criada e encerrada rapidamente).

**Conclusão: a thread principal roda em precisão de 53 bits (0x27F: máscara total, arredondamento ao mais próximo, PC=double), nunca 24 bits.** Nenhuma thread além da 1 chamou o porteiro nem `sim_advance`. O `rt2-oracle` deve escrever `FPCW=0x27f` (já documentado).

Segunda execução (420 s, com um jogo carregado pelo usuário via Load Game e deixado rodando): 1199 amostras do porteiro, 1470 de `sim_advance`, **63 chamadas da receita `0x4bf760`**, todas `fctrl=0x27f tid=1`. Exemplo: `REVENUE fctrl=0x27f tid=1 eip=0x4bf760 date=0x409074e tod=0x3a06 rng=0x6f0146ec` (ano 1870, mês 9, dia 4).

Sem `xdotool`/`ydotool` (só `wtype`, que não clica), o carregamento do jogo é manual: Load Game, `games/Tutorial.GAM`, despausar, deixar correr alguns minutos e **não fechar a janela** até o `probe.py` terminar (ele mata o jogo ao fim do tempo dado; use 300+ s).

## 4. Pendências para o nível 2 (save como oráculo)
Automatizar cliques (instalar `ydotool` ou usar mensagens Win32 injetadas pelo gdb: chamar a rotina de carga de jogo a partir de um breakpoint), avançar N dias e salvar em `<sandbox>/saves`.
