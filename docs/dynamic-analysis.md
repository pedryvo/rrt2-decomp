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
- **Entre sandboxes (caminhos diferentes) os bytes também diferem** nos mesmos campos: ponteiros do heap gravados crus (registros de trem, parte fixa das regras, um campo de pessoa) e o checksum. A lista exata e versionada está em `rt2_formats::savefields::MASKS` (seção 5.5); `rt2-savediff` mascara por padrão.
- O passo de movimento (`--step`) altera o resultado dos trens, pois `sim_advance` usa `dt` variável (ver `time.md` seção 9); o oráculo fixa 320 min, e o clone precisa usar o mesmo.
- Não roda o resto de `game_frame_update` (`0x405240`, `0x405390`, `0x406080`: UI/ambiente) nem comandos de jogador; só um jogador local; a velocidade do arquivo (3) não importa.
- Uma instância por vez (usa e mata o `winedbg`/`wineserver` do prefixo do sandbox).

### 5.5 `rt2-savediff`: comparação campo a campo e campos voláteis

`rt2-savediff a.GAM b.GAM [--no-mask] [--only <prefixo>] [--json]` (`crates/rt2-tools/src/bin/rt2-savediff.rs`) imprime os campos diferentes (`caminho: a -> b`) agrupados por entidade (`state`, `trains[3]`, `tiles`...), com um resumo por seção; código de saída 0 se iguais depois das máscaras, 1 se diferem. A lista de campos vem de `Save::fields()` (`crates/rt2-formats/src/savefields.rs`): **todos os bytes** do arquivo, na ordem do arquivo, com nomes estáveis (`state.date`, `places[12].name`, `tiles[x,y].rest`, `pool_78.next[i]`) para o que está decodificado e pedaços crus de até 4 bytes alinhados (`trains[3].raw[0x40..0x44]`, offset dentro do registro/bloco) para o resto. Índices de pool são o número do slot; registro presente só em um dos lados aparece como `<absent>`. Entre versões diferentes (0x233C contra 0x2348 do mesmo mundo) só os campos nomeados são comparados (os crus e as propriedades do layout, como `record_size` e os 2 primeiros bytes do tile, que o loader remapeia, são ignorados). O Example original contra o mesmo avançado 0 dias no original difere só em `pool_58` (a conversão no load acrescenta 10 registros: 72 -> 82).

Máscaras (`MASKS`, por versão; todas verificadas comparando saves iguais gerados em sandboxes diferentes, que ficam idênticos depois delas):

| Campo | Offset | Por quê |
|---|---|---|
| `header.checksum` | +0x86F7 | checksum do buffer inteiro, cobre os bytes voláteis abaixo |
| preview | `header` +0x387..+0x7474 | imagem 170x170 desenhada da tela ao salvar; muda com o estado da janela (só varia em sandbox ainda não aquecido) |
| `trains[*]` | +0x37..+0x3B e +0x45..+0x59 | 6 ponteiros do heap gravados crus (valores `0x02xxxxxx` num sandbox, `0x03xxxxxx` em outro) |
| `rules[*].fixed` | +0x00, +0x08..+0x10, +0x50..+0x60 | ponteiros do heap crus na parte fixa da regra (`0x02xxxxxx`/`0x01xxxxxx`) |
| `persons[*]` (só 0x2348) | +0x54C..+0x554 | campo novo do registro: um ponteiro do heap e um número pequeno; muda entre sandboxes |

(O aviso anterior sobre "ponteiros em registros de trem" só valia para o Example; no Tutorial, que não tem trens, os ponteiros estão nas regras.)

Teste: `RT2_SAVE_2348=a.GAM:b.GAM cargo nextest run -p rt2-formats savefields` (cada arquivo contra si mesmo é igual; dois dias do mesmo mundo diferem em `state.date`).

### 5.6 O que muda ao avançar o mundo (amostras do oráculo)

Medido com `rt2-savediff` sobre saves gerados por `advance_save.py` (semente 1234, passo 320, sandbox aquecido; depois das máscaras não sobra ruído). Campos citados por caminho, offsets dentro do registro; "registros" = slots que mudaram.

- **Tutorial, 0 -> 1 dia** (1870-02-04 -> 02-05): só `state.game_time_minutes` (+1440) e `state.date`. Nada mais muda: sem trens, sem RNG consumido.
- **Tutorial, 1 -> 7 dias** (-> 02-11): além do estado, 4 registros de `objects` (pool `c74`, +0x4C u32 crescente, +0x54 u32, +0x64 e +0x88 passam de 0 a 1) e **4 registros novos em `pool_7c`** (`c7c`, 0x15 bytes: contagem 150 -> 154, `field_04` e bitmap acompanham).
- **Tutorial, 7 -> 30 dias** (-> 03-06, cruza o mês): `state.raw[0x230..0x240]` (3 dwords), `objects` (+0x4C, +0x54, +0x64, +0x88, 86 campos) e `pool_7c` (129), e `persons[1..4].raw[0x12c..0x130]` (um u32 por pessoa, só no virar do mês).
- **Example, 0 -> 1 dia** (1875-12-10 -> 12-11), 1200 campos em 277 registros: `state` (2), `tiles[x,y].rest` em 7 tiles (struct_id e o byte seguinte: estruturas novas), `structures` (+8 registros: `count`, `field_04`, bitmap, e ids novos/removidos), `pool_70` (0x4F: 25 registros, +0x14..+0x20, +0x30..+0x40), `pool_78` (0x23, lista ligada: 129 registros, `next`/`prev` mudam: registros criados/removidos a cada dia), `pool_80` (6), `track_pieces` (27 campos: +0x04..+0x0C, +0x14..+0x24) e `trains` (92 campos em 9 trens: +0x60 u32, +0x69..+0x70, +0x98, +0xBC, +0xC8, +0xF8 e as listas `route`/`cars` do trem em movimento). Nenhuma mudança em `stations`, `companies`, `persons`, `places` nem `elevation`.
- **Example, 1 -> 7 dias**: os mesmos conjuntos, 3201 campos em 497 registros; entra `stations` (351 campos em 10 estações: +0xBC/+0xC0, +0x168, +0x320..+0x334; significado ainda não decodificado), `objects` (40), `pool_7c` (51), `elevation` e `tiles` (8).
- **Example, 7 -> 30 dias** (-> 1876-01-12, cruza o ano): 8277 campos. `state` muda 1105 dwords crus (tabelas mensais/anuais em `state.raw[0x598..0x5c4]`, `0x718..0x7b4`, `0x1254..0x12f4`, `0x2aa0..0x3824` etc.), `header.raw[0x88e8..0x896c]` (19 dwords: as 53 entradas de 9 bytes de `0x5702cb` passam de 100 para 500, `0x64 -> 0x1f4`, no virar do ano), `companies` (820 campos, ex.: `companies[4].raw[0x134..0x148]`, `0x664`), `objects` (1906: +0x34, +0x48, +0x64, +0x68, +0x90, +0x94 em 342 registros), `places` (113: um dword em +0x2C, dentro dos 32 bytes do campo do nome, em 88 lugares, e +0x3C..+0x44 em 8), `persons` (74), `stations`, `track_pieces` (438), `territories`, `elevation` (4 cantos) e `tiles` (48).
- **Mesmo mundo, sandbox diferente** (Example 7 dias, `rt2sandbox` e `rt2sandbox3`): 0 diferenças depois das máscaras (6951 campos mascarados, 9 trens x 8 + 6 + regras + checksum + pessoa).

Implicações para o tick do mundo: a cada dia só `state.game_time_minutes`/`date` são certos; o resto (estruturas, `pool_78`/`pool_70`/`pool_7c`, `objects`, trens, estações) muda por regras ainda não especificadas; os campos mensais/anuais do estado e do cabeçalho só mudam ao cruzar o mês/ano.

### 5.7 `scripts/fidelity.py`: placar de fidelidade automatizado

`python3 -I scripts/fidelity.py [--refresh] [--only NOME] [--no-build] [--no-write]` roda todos os cenários de `scripts/fidelity-scenarios.toml` (save relativo à instalação, lista de dias, semente) e reescreve a tabela de `docs/fidelity-status.md` entre `<!-- fidelity:begin -->` e `<!-- fidelity:end -->` (o resto do documento é à mão).

- Por caso: `base` = original com 0 dias (conversão para 0x2348), `orig_N` = original com N dias (`advance_save.py`, passos de 320 min), `clone` = `rt2-advance base N semente`, e `rt2-savediff --json orig_N clone` dá iguais / N campos / três maiores grupos.
- Saídas do original ficam em cache (`~/.cache/rrt2/fidelity/<sha256 do RT2_PLAT.EXE.orig>/`, chave: cenário + sha do save + semente + dias; sandbox do Wine em `~/.cache/rrt2/fidelity/wine/`), então o Wine (~12 s por run, ~1 min no primeiro por causa do aquecimento do sandbox) só roda uma vez; `--refresh` refaz. Antes de cada run espera (até `--wait`, 900 s) não haver `winedbg` ativo, porque `advance_save.py` mata todos os `winedbg` da máquina.
- O estado anterior (casos IGUAIS) fica num comentário `<!-- fidelity:state ... -->` dentro dos marcadores; o script sai com 1 se um caso que estava igual deixar de ser (e mantém a linha de base igual até ele voltar).
- Só `.GAM` e `.MP2` (o `.MP2` é lido copiado como `.GAM`); os `maps/#NN*.MAP` precisam de semântica de New Game que o clone não tem e ficam de fora.

## 6. Snapshots de uma função (teste diferencial no nível da função)

`re/scripts/wine/snap_call.py <sandbox> <in.GAM> <dias> <semente> <0xfunção> <K> <nome> <dir> [função_do_clone]` roda o original como `trace_save.py` (agora com `TR_GDBPY` para trocar o script do gdb) e, na K-ésima chamada da função, **grava o jogo antes da chamada e depois do retorno** (`snap_gdb.py`: o gravador `0x477540` é chamado com a pilha 0x3000 abaixo do quadro corrente e os registradores são restaurados; a FPU está vazia na entrada das funções medidas). Registra o trem/vagão de `ECX` (slot), o RNG do jogo (`0x55d2c0`) e o rand do CRT (`0x5609f0`) antes e depois, e acrescenta a linha `nome função trem rng crt rng_depois crt_depois [dt]` a `<dir>/manifest.txt`. Com `SNAP_CALLS=<arquivo>` e `SNAP_FUNCS=<json>` o script registra também a árvore de chamadas feita durante a chamada (profundidade pela pilha), útil para saber quais funções portar.

O teste `crates/rt2-sim/tests/trainstep_snapshots.rs` lê `$RT2_FIDELITY_DIR/snaps/manifest.txt`, carrega cada `_pre.GAM`, roda o port da mesma função com os mesmos estados de RNG e compara com `_post.GAM` campo a campo (e os estados de RNG finais). Assim um caminho raro (chegada à estação, quebra, troca de parada) é verificado mesmo quando o resto do mundo do clone ainda diverge do original naquela data. Snapshots atuais (Example, semente 1234, até 120 dias): `train_step` em dois instantes dos dias 37 e 74, duas chegadas com entrega e receita (`0x4d29e0`), a carga na estação (`0x4d1bb0`), a saída da estação (`0x4d5560`), custos mensais (`0x4d4390`) e passo anual (`0x4d4620`): todos idênticos.

`re/scripts/wine/snap_train.py <sandbox> <in.GAM> <semente> <nome> <dir> <trem> <dt> <passos,csv>` (com `CALL_MARK=1` o trem é marcado antes, como o comando `0x2c`) chama `train_step` (`0x4d0ce0`) do trem repetidas vezes (o relógio avança `dt` antes de cada chamada) e grava o jogo depois de cada contagem de passos; as linhas `nome_sN 0x4d0ce0 trem rng crt rng_depois crt_depois 0 dt N` do manifesto são lidas por `tests/build_snapshots.rs` (casos `tp*`: colocação e movimento de um trem comprado, `docs/spec/trains.md` 12). Cuidado: `advance_save.py` e os scripts de snapshot matam todo `winedbg`; duas execuções em paralelo se derrubam.
