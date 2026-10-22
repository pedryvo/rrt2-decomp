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

## 5. `advance_save.py`: avançar um save N dias no original (oráculo de nível 2)

Ferramenta headless, sem GUI e sem cliques: `re/scripts/wine/advance_save.py`, com o lado do gdb em `advance_gdb.py` e a validação em `advance_check.py`.

```
re/scripts/wine/run_original.sh $SB --no-run
re/scripts/wine/advance_save.py $SB games/Tutorial.GAM <dias> <semente> <saida.GAM> [--step 320] [--timeout 240]
re/scripts/wine/advance_check.py $SB <Tutorial.GAM> <pasta>      # 1/7/30 dias, determinismo, semente
```
A entrada é copiada para `$SB/games/_advin.GAM` (a instalação nunca é tocada). A saída é gravada em `$SB/saves/_advout.GAM` e copiada para `<saida.GAM>`. Só `.GAM` (o `.MAP` não foi tentado). Tempo: **~11-13 s por run**, quase tudo é a inicialização do Wine; o avanço de 30 dias leva menos de 1 s.

### 5.1 Sequência de chamadas (processo do original, via gdb)
O gdb para no porteiro `game_idle_tick` `0x404e30` com o jogo no **menu principal** (`0x566e14 == 0xb`, depois de 30 quadros). Cada chamada é feita por "register+jump": escreve-se o endereço de retorno `0x4a5940` (código morto) e os argumentos na pilha, ajusta-se `ECX`/`EDX`/`ESP`/`EIP`, `continue`, e lê-se `EAX` quando o breakpoint de retorno dispara. Todas são `fastcall` (ECX, EDX).

| Passo | Chamada | Observação |
|---|---|---|
| 0 | `_malloc(0x400)` `0x51b6b7` (cdecl) | buffer na heap do jogo para os nomes |
| 1 | `[0x574c04] = 1` | **bloqueia o porteiro**: as rotinas de UI de carga/gravação chamam o porteiro de forma aninhada e, sem isto, o jogo avança sozinho (+160 min medidos) |
| 2 | `[0x57fee0]++`, `[0x57bc34]++`, `[0x57b41c]++`; `0x495700(ECX=0)` | mesmo prólogo do botão Load Game do menu (`0x4b75b0`, caso `0xc57`, rótulo `0x4b8c9a`) |
| 3 | `0x479710(ECX="_advin", EDX=0)` | monta `games\_advin.GM2`/`.GAM` e chama o carregador `0x477a60`; o jogo passa a `mode=1`, `running=1`, cria a janela `0xfd` |
| 4 | desfaz os contadores; `0x502200(1,0)`, `[0x574bc4]=1`, `0x4f6130(1)` | epílogo do mesmo caso do menu |
| 5 | `[0x55d2c0] = semente`, `[0x5609f0] = semente` | RNG do jogo e `rand()` do CRT; a carga **não** restaura a sequência (e consome RNG, deixando-o em outro valor) |
| 6 | `sim_advance 0x4a5cb0(ECX=dt)` em passos de `--step` min (padrão 320 = 100 ms à velocidade 3), total `1440*dias` | sem porteiro e sem UI; o script confere que `[0x5704f4]` avançou exatamente `1440*dias` |
| 7 | `0x477540(ECX="saves\\_advout.GAM", EDX=0)` | gravador; EDX=0 grava em disco |

Nenhum diálogo modal apareceu em 30 dias do Tutorial. Se um evento abrir um (não tratado), o `sim_advance` não retorna e o `--timeout` mata a sessão com a mensagem TIMEOUT.

### 5.2 Formato gravado
O gravador **sempre** grava a versão `0x2348` (cabeçalho `0x898a`, bloco de estado `0x3a74`), que `Save::parse` ainda não lê (só `0x233C`). Por isso `advance_check.py` lê a data direto: estado em `4 + 0x898a + 0x86 + 0x40 = 0x8A54` (+0xC minutos, +0x10 data, +0x14 hora). Um parser `0x2348` é pré-requisito para o diff campo a campo.

### 5.3 Resultados (Tutorial, 1870-02-04 19:03, semente 1234)
```
1 dia   -> 1870-02-05 19:03   (esperado igual)   minutos 904828023
7 dias  -> 1870-02-11 19:03   (esperado igual)   minutos 904836663
30 dias -> 1870-03-06 19:03   (esperado igual)   minutos 904869783   (meses de 28 dias)
30 dias, semente 1234 duas vezes -> arquivos byte a byte idênticos
30 dias, semente 777 -> arquivo diferente (11 bytes); RNG final 0x51bc859 contra 0xa4a5e93c
```
Em 1 e 7 dias o RNG do jogo não é consumido (fica na semente); o primeiro consumo no Tutorial aparece no passo mensal.

### 5.4 Limitações
- **Aquecimento do sandbox:** nos primeiros runs de um sandbox o jogo cria/reescreve `rt2_tsc.CFG` e `ddraw.ini`, e o save sai com bytes diferentes (ponteiros do heap em registros de trens, um u32 em ~0x117FAD no Tutorial, preview e checksum). `advance_save.py` repete runs de 0 dias até dois seguidos serem idênticos (marcador `.advance_warm`), uma vez por sandbox (~35 s). Depois disso, determinismo byte a byte.
- **Entre sandboxes (caminhos diferentes) os bytes também diferem** nos mesmos campos: ponteiros do heap gravados crus nos registros de trem (`+0x45..+0x55` das listas de passos) e o checksum. A comparação com o clone deve mascará-los.
- O passo de movimento (`--step`) altera o resultado dos trens, pois `sim_advance` usa `dt` variável (ver `time.md` seção 9); o oráculo fixa 320 min, e o clone precisa usar o mesmo.
- Não roda o resto de `game_frame_update` (`0x405240`, `0x405390`, `0x406080`: UI/ambiente) nem comandos de jogador; só um jogador local; a velocidade do arquivo (3) não importa.
- Uma instância por vez (usa e mata o `winedbg`/`wineserver` do prefixo do sandbox).
