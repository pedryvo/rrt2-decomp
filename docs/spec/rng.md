# Geradores de números aleatórios

Fonte: `RT2_PLAT.EXE`, lido com Ghidra via REA. Estado das provas: **[V]** = confirmado contra o original por teste diferencial no oráculo (`crates/rt2-oracle/tests/sim_time_rng.rs`); **[S]** = lido no código. O manual não descreve o RNG.

## 1. Resumo

| # | Gerador | Algoritmo | Estado | Função de passo | Uso |
|---|---|---|---|---|---|
| A | **RNG do jogo** | LCG ANSI C: `s = s*0x41c64e6d + 0x3039` (mod 2^32), saída `(s >> 16) & 0x7fff` (shift aritmético) | `0x55d2c0` (u32) | `rng_range` `0x4f0660` [V] | simulação: terreno, eventos, trens, empresas, cidades (40 chamadores) |
| B | **Hash com semente** (sem estado) | `(0x3039 - seed*0x3e39b193) >> 16 & 0x7fff` | nenhum (a semente vem do chamador) | `rng_hash_range` `0x4f06f0` [V] | geração de terreno e decoração a partir de `game_seed` e posição |
| C | **rand() do CRT** (MSVC) | LCG `s = s*214013 + 2531011`, saída `(s >> 16) & 0x7fff` | `0x5609f0` (u32) | `crt_rand` `0x51c137` [V], intervalo em `crt_rand_range` `0x4f0790` [V] | áudio, UI, partículas, ids e sementes de sessão; **semeia** o jogo ao criar partida |
| D | LCG local de dissolução | `s = s*0x41c64e6d + 0x3039`, usa `s >> 16` | argumento de `0x4f6540` | inline em `0x4f663f` [S] | efeito visual (cópia de pixels), fora da simulação |

Os geradores A e C têm estados e rotinas separados; A é determinístico dado a semente (e é o que importa para a fidelidade), C é semeado por `GetTickCount` e não é reprodutível.

## 2. Gerador A: RNG do jogo

### 2.1 Passo
```
proximo15(s):                      ; 0x4f06a3..0x4f06c4
    s = s * 0x41c64e6d + 0x3039    ; u32 com wrap
    devolve (i32(s) >> 16) & 0x7fff
```
Mesmas constantes do rand() de glibc/ANSI (1103515245, 12345). Os 16 bits baixos do estado nunca saem; a saída tem 15 bits.

### 2.2 `rng_range(lo, hi)` (`0x4f0660`, fastcall: ECX = lo, EDX = hi) [V]
Intervalo **inclusivo** `[lo, hi]`:
```
span = hi - lo                                   ; i32
se span > 0x7fff:                                ; sinal: jle 0x4f069f
    a = rng_range(0, 0x7fff);  b = rng_range(0, 0x7fff)      ; nessa ordem
    devolve ((b | (a << 15)) % (span + 1)) + lo   ; divisão u32
se hi == lo: devolve hi                          ; NÃO avança o estado
v = proximo15(estado)
devolve (v % (span + 1)) + lo                    ; idiv com sinal
```
Observações:
- Intervalos grandes (> 32767 valores) consomem **duas** saídas (30 bits).
- O módulo introduz viés (sem rejeição); o clone deve reproduzi-lo.
- `hi < lo` não é tratado: `span + 1` é negativo ou zero. Com `span + 1 == 0` (`hi == lo - 1`, por exemplo `rng_range(0, n - 1)` com `n == 0`) o `idiv` gera exceção de divisão por zero (falha do jogo); os testes não geram esse caso.
- Funções auxiliares: `rng_seed(s)` `0x4f06d0` grava o estado; `rng_get_state()` `0x4f06e0` lê [V].

Verificação: 30 000 chamadas com faixas variadas (incluindo `lo == hi`, negativas e > 0x7fff), com re-semeadura periódica pelo setter original; valor devolvido e estado em `0x55d2c0` comparados a cada chamada (teste `rng_step_range_matches_original`).

### 2.3 Referência em Rust (a mesma do teste)
```rust
fn next15(state: &mut u32) -> i32 {
    *state = state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
    ((*state as i32) >> 16) & 0x7fff
}
fn range(state: &mut u32, lo: i32, hi: i32) -> i32 {
    let span = hi.wrapping_sub(lo);
    if span > 0x7fff {
        let a = range(state, 0, 0x7fff) as u32;
        let b = range(state, 0, 0x7fff) as u32;
        return ((b | (a << 15)) % (span as u32 + 1)).wrapping_add(lo as u32) as i32;
    }
    if hi == lo { return hi; }
    next15(state) % (span + 1) + lo      // span + 1 == 0 divide por zero no original
}
```

### 2.4 Semente e re-semeadura (todos os chamadores de `rng_seed` / `rng_get_state`)

| Onde | Endereço | Regra |
|---|---|---|
| Novo jogo | `0x410de0` (`0x410f79..0x410f89`) | `rng_seed( crt_rand_range(1, 32000) + clock_ms() )`. Antes (`0x410f08..0x410f64`): `game_seed (0x57486e) = crt_rand_range(1, 32000) + clock_ms()`. As duas sementes são independentes. [S] |
| Início sincronizado em rede | `0x4b69b0` (`0x4b6cbf`) | `rng_seed(game_seed)` [S] |
| Comando de rede | `cmd_execute` `0x4b0210` (`0x4b02a4`) | `rng_seed(arg)` com a semente enviada pelo host [S] |
| Todo dia, só em rede (jogadores > 1) | `sim_advance` `0x4a5cb0` (`0x4a5e8f..0x4a5ea8`, `0x4a601a..0x4a6037`) e a cópia morta `0x4a5940` | `rng_seed( ftol( (f64)(game_time_minutes + game_seed) + campo f64 em +0x12a do registro 0 da lista de `0x46a8a0` ) )` [S] |
| Decoração determinística do mapa | `0x471190` | salva o estado (`0x4711c3`), `rng_seed(123)` (`0x7b`), sorteia, restaura (`0x47135e`) [S] |
| Preservar a sequência ao iniciar o mundo | `0x477a60` (`0x477c75..0x477c8f`) | `s = rng_get_state(); 0x472980(...); rng_seed(s)`; idem num segundo trecho; o mundo é gerado sem consumir a sequência do jogo [S] |
| Checksum de sincronia | `net_sync_checksum_build` `0x46c680` | grava `rng_get_state() & 0xffff` no registro de checksum (detecção de dessincronia em rede) [S] |

Em jogo local a sequência vem apenas da semente inicial (aleatória por relógio) e das chamadas em ordem; **a ordem das chamadas é parte do estado**. Para reproduzir um jogo é preciso a mesma semente e a mesma ordem de atualização das entidades (ver `time.md`).

### 2.5 Chamadores de `rng_range` (40 funções) [S]
Faixas imediatas vistas junto à chamada (heurística: o decompilador não resolve todos os argumentos; `N` = contagem de uma lista). Detalhes ficarão em cada spec de subsistema.

| Chamador | Faixas vistas | Observação |
|---|---|---|
| `0x40ba00` | [1,24] | |
| `0x40bb40` | [0,100] | |
| `0x414580` | [0,5] | |
| `0x424a40` | [0,100] x2, [0,9999] | |
| `0x4259e0`, `0x42a7d0` | [0, N-1] | N = campo em `+0x540b04` |
| `0x429fc0` | [0,100], [0,31], [0,5], [0,3], [0,8] | |
| `0x42a3e0`, `0x42fcb0`, `0x42ffc0`, `0x438a10`, `0x48cc00` | variáveis | |
| `0x42ee00`, `0x48b5d0` | [-20,20] e variáveis | |
| `0x42f6f0` | variável | |
| `0x4302c0` | [88,112] | |
| `0x430540` | [80,105]/[95,105], [0,1000000] x3 | comparado com `game_time_minutes` |
| `0x431660` | [0, N-1] | N = contagem da lista `0x574c5c`; sorteio mensal |
| `0x435320` | [80,120] | |
| `0x435c20` | [70,130], [1,10] | |
| `0x436b30` | [180,360] | |
| `0x438450` | [25,180], [1000,12000], [0,39] x2 | |
| `0x43c760` | [20,110], [-200,200] x2 | |
| `0x43dca0` | [30,90] | |
| `0x43e040` | [0,100] x2 | |
| `0x454ae0` | variáveis | também usa `crt_rand_range` (seção 4) |
| `0x46a010` | [-5,5], [0,10] | passo de 6 meses (meses 2 e 8) |
| `0x471190` | [0,100], [-d,d] | decoração (semente 123) |
| `0x472980` | [0,1000] | criação do mundo |
| `0x474280` | [-32767,32767], [-80,80] | |
| `0x4a5320` | [0,10000] | sorteio mensal com a janela em torno de 1860 (`time.md`, 7.4) |
| `0x4a6a60` | [0,40] | |
| `0x4a6b00` | variáveis | |
| `0x4a7960`, `0x4a7990` | [60,450], [0,40] | |
| `0x4ce4c0`, `0x4ce730` | limite inferior 60 e 30 (superior não resolvido) | |
| `0x4ceaa0` | [30,60] | |
| `0x4cf390` | [0,1000000] x2, [0,100] x2 | movimento de trens |
| `0x4d86f0` | [80,120] | passo anual |
| `0x4d87e0` | [75,125] | |

## 3. Gerador B: hash com semente (`rng_hash_range`, `0x4f06f0`) [V]
Fastcall: ECX = semente, EDX = lo, pilha = hi (`ret 4`).
```
span = hi - lo
se hi < lo: (ramo de versão: 0x49d920 > 0x232c devolve lo)    ; 0x4f0700
se span > 0x7fff:
    a = hash(seed, 0, 0x7fff);  b = hash(seed + 7, 0, 0x7fff)
    devolve ((b | (a << 15)) % (span + 1)) + lo               ; u32
se hi == lo: devolve hi
t = 0x3039 - seed * 0x3e39b193                                 ; i.e. 0x3039 + seed * 0xc1c64e6d
devolve (((t as i32) >> 16) & 0x7fff) % (span + 1) + lo       ; idiv com sinal
```
Chamadores: `0x428fe0`, `0x4294d0`, `0x430fd0`, `0x4a3540`, `0x4a3800`, `0x4a8800`, `0x4b3ec0`, `0x4b4360`, `0x4bf760`. Em `0x4b3ec0` e `0x4b4360` a semente é derivada de `game_seed` (`0x57486e`); nos demais é calculada pela posição (terreno, decoração). O teste `rng_hash_matches_original` usa 20 000 casos com `hi >= lo`.

Há duas outras sequências de hash por posição **inline** que multiplicam por `0x41c64e6d` (`0x41f594`, `0x42a492`): provável variação por tile; ver "Em aberto".

## 4. Gerador C: rand() do CRT

- `crt_srand` (`0x51c12d`) grava `0x5609f0`; `crt_rand` (`0x51c137`): `s = s*214013 + 2531011; devolve (s >> 16) & 0x7fff` [V]. É o `rand()` do MSVC.
- Semeado com `GetTickCount()` em `game_start` (`0x49eff0`) e de novo em `net_session_init` (`0x4efd50`), que também sorteia um id de sessão: `((rand % 32000 + 1) << 16) | (rand % 32000 + 1)` somado a `GetTickCount()`. `0x4ef570` (chamado de `0x44dc90`, inicialização) também usa `crt_rand` e `GetTickCount`.
- `crt_rand_range(lo, hi)` (`0x4f0790`, fastcall) [V]:
```
se hi == lo: devolve hi;  se hi < lo: devolve lo
span = hi - lo
se span > 0x7fff:  a = crt_rand(); b = crt_rand(); devolve (((a & 0x7fff) << 15 | (b & 0x7fff)) % (span + 1)) + lo
devolve crt_rand() % (span + 1) + lo
```
Chamadores (18): `0x406470`, `0x411150` (áudio), `0x410de0` (sementes do jogo), `0x412a90`, `0x44dc90`, `0x44f730`, `0x44fac0`, `0x454ae0`, `0x47d100` (áudio), `0x49de20`, `0x49fe80`, `0x4a0410`, `0x4a42f0`, `0x4b9ff0`, `0x4d0400`, `0x4d29e0`, `0x4f56f0`, `0x4f5ca0`. `0x4d0400`, `0x4d2360` e `0x4d29e0` sorteiam `[0,3]` e estão no caminho do movimento de trens (`0x4d0ce0`): precisam ser investigados, pois um sorteio do CRT em lógica de simulação a torna não reproduzível.

Verificação: 20 000 chamadas (rand direto e intervalo) com re-semeadura; estado comparado a cada chamada (`crt_rand_matches_original`).

## 5. Divergências e fidelidade
- Manual: sem informação de RNG. Nada a anotar.
- Fidelidade: o clone deve usar A com o mesmo algoritmo, a mesma semente de partida (ou a de um `.GAM`, onde o estado `0x55d2c0` precisa ser lido do arquivo: ver "Em aberto") e a mesma ordem de chamadas. B é puro. C não pode ser reproduzido; onde afeta o estado de simulação (suspeita em `0x4d0400`, `0x4d2360`, `0x4d29e0`) isso será uma divergência documentada.

## Em aberto
- Resolvido (docs/formats/map-gam.md 5.2): `game_seed` fica no `.GAM` em 0xB24F; o estado `0x55d2c0` **não é gravado** (o load preserva o do processo), então o teste de nível 2 precisa re-semear o RNG explicitamente após carregar.
- Papel de cada chamador de `rng_range` e se os de `crt_rand_range` em `0x4d0400`, `0x4d2360`, `0x4d29e0`, `0x412a90`, `0x4a0410`, `0x4a42f0` alteram estado de simulação.
- As sequências inline com multiplicador `0x41c64e6d` em `0x41f594` e `0x42a492` (dois pontos; hash de posição para variação de tile).
- Registro apontado por `0x46a8a0(0)` e o campo f64 `+0x12a` usados na re-semeadura diária em rede.
- Condição do ramo `hi < lo` de `rng_hash_range`: `0x49d920` (versão mínima dos jogadores em rede; 9012 em jogo local) só é consultada nesse ramo; não testado.
