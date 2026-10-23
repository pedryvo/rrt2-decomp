# Protocolo de rede do clone (`rt2-net`)

Formato das mensagens do multiplayer entre cópias do clone. As **regras** vêm do original (`docs/spec/multiplayer.md`); o transporte e a codificação são do clone: não há DirectPlay nem compatibilidade de rede com o `RT2_PLAT.EXE`. Implementação: `crates/rt2-net` (`wire.rs`, `msg.rs`, `link.rs`, `transport.rs`, `session.rs`).

Convenções: tudo em little-endian; `u8/u16/u32/u64/i32`; `bool` = `u8` (0/1); `bytes` = `u32` tamanho + dados; `str` = `bytes` em UTF-8; `[N]` = N bytes crus. Uma mensagem lida deve ser consumida inteira (bytes sobrando = malformada). Tipo desconhecido, texto que não é UTF-8, contagens absurdas (mais de 64 jogadores, mais de 100 000 comandos num lote) e chat com mais de 900 caracteres são descartados com `Event::Protocol`, nunca causam pânico.

**Versão**: `PROTOCOL_VERSION = 1` (em `msg.rs`). Sobe quando qualquer formato abaixo muda; pares com versões diferentes se recusam no `Hello`. Os ids de comando são os do original (`docs/spec/commands.md`) e nunca mudam de significado; um comando novo ganha uma tag nova e o `match` exaustivo em `wire.rs` não compila sem ela.

## 1. Transporte

Um `Transport` leva **quadros** opacos por **ligações** (`LinkId`) e entrega eventos `Opened`, `Closed` e `Data`. Pode perder, duplicar, atrasar e reordenar (como os datagramas do DirectPlay); nunca lê relógio (`poll(now_ms)` recebe o tempo do chamador). Implementações:

- `mem::MemHub` / `MemTransport`: vários nós no mesmo processo, tempo lógico, gerador semeado para os defeitos (`Faults`: latência, jitter, perda e duplicação em milésimos), `cut` (cabo puxado: sem evento) e `kill` (a ligação fecha com evento). Endereço `mem:<nome>`.
- `tcp::TcpTransport`: `std::net` + uma thread leitora por ligação + uma thread que aceita. Quadro = `u32` tamanho + bytes (máximo 256 MB: um save viaja num quadro só). Endereço `ip:porta`; conexão direta, sem descoberta.

### 1.1 Camada confiável (`link.rs`)

Todo quadro tem um byte de tipo:

| tag | quadro | conteúdo |
|---|---|---|
| `0` | cru | a mensagem (batimento, sonda, ping, pong, chat: o original também não confirma) |
| `1` | dado | `id u32` + a mensagem; reenviado a cada `resend_ms` (3 s no original) até chegar o ack |
| `2` | ack | `id u32` |

O receptor entrega cada `id` uma vez, na ordem de chegada (a ordem de aplicação é problema do `seq` dos lotes), confirma toda cópia e guarda só os ids acima do piso contíguo.

## 2. Mensagens

Primeiro byte = tipo (`07` a `11` coincidem com os tipos do original, `multiplayer.md` 1.3; o chat do original é o `04`, aqui `16`; o `04` daqui é o roster). "Confiável" = vai em quadro `1`.

| tipo | nome | confiável | corpo |
|---|---|---|---|
| `01` | `Hello` | sim | `"RT2N"[4]`, `protocolo u16`, `rules_hash u64`, `identidade u32`, `nome str`, `endereço str`, `é_malha bool`, `id_malha u32` |
| `02` | `Welcome` | sim | `seu_id u32`, `id_do_anfitrião u32`, `em_andamento bool`, jogadores |
| `03` | `Reject` | sim | `motivo str` |
| `04` | `Roster` | sim | `id_do_anfitrião u32`, jogadores |
| `07` | `Cmd` | sim | um comando (cliente para anfitrião; o remetente é o dono da ligação) |
| `08` | `Batch` | sim | `seq u32`, `stamp i32`, `horizonte i32`, `n u32`, n × (`remetente u32`, comando) |
| `09` | `HostChanged` | sim | `novo_anfitrião u32` |
| `0a` / `0b` | `Ping` / `Pong` | não | `token u64` (o ping é `agora - token`) |
| `0c` | `Leave` | sim | vazio |
| `0d` | `Heartbeat` | não | vazio |
| `0e` | `Probe` | não | vazio (responde `Heartbeat`) |
| `0f` | `Sync` | sim | `minuto i32`, digest[64] |
| `10` | `Snapshot` | sim | ver 2.3 |
| `11` | `Loaded` | sim | vazio |
| `14` | `Desync` | sim | `minuto i32`, digest do anfitrião[64], `grupos u8` |
| `15` | `StateDump` | sim | `minuto i32`, digest[64], `save bytes` (vazio sem pools) |
| `16` | `Chat` | não | `texto str` (até 900 caracteres) |

**jogadores** = `n u32` (no máximo 64) e n × (`id u32`, `identidade u32`, `nome str`, `endereço str`). O `endereço` é onde os outros se conectam a ele (malha, migração).

**digest** (64 bytes) = `rng u32`, `relógio i32`, depois 7 × `u64`: roster, empresas, pessoas, trens, indústrias, estações, resto. Cada `u64` é FNV-1a 64 (`rt2_sim::net::Fnv`) sobre os bytes do grupo (`rt2_sim::net::digest` descreve o que entra).

### 2.1 Conexão e início

1. O convidado liga ao anfitrião e manda `Hello` (`é_malha = false`). O anfitrião confere `protocolo` e `rules_hash` (`rt2_net::rules_hash`: versão do protocolo, versão e `RULES_REVISION` do `rt2-sim`, hash do executável de onde vieram os dados, bits do conjunto de correções de bugs). Diferença = `Reject`. Mais de `max_players` (32 como no original) = `Reject`.
2. No lobby o anfitrião responde `Welcome` (`em_andamento = false`) e manda `Roster` a todos. Os ids são atribuídos em ordem crescente (`max + 1`); o roster é ordenado por id e o menor id depois do anfitrião é o sucessor.
3. O convidado liga a todos os outros jogadores (endereços do `Welcome`) e manda `Hello` com `é_malha = true` e o próprio id, para que cada par saiba de qual jogador é a ligação. É uma malha completa; só o anfitrião manda lotes.
4. O anfitrião inicia (`Session::start`): manda a cada convidado um `Snapshot` tipo `Start`. Todos (o anfitrião também) carregam o instantâneo, rodam `World::net_begin` (RNG com `game_seed`, atribuição de pessoas) e o convidado responde `Loaded`. Quando todos responderam o anfitrião começa a emitir lotes.

### 2.2 Lotes e comandos

Comando = tag `u8` + campos na ordem de declaração. Tags de `Command`: `01` velocidade absoluta (`u8`), `02` velocidade relativa (`i8`), `03` pausa, `04` semear RNG (`u32`), `05` semear CRT (`u32`), `06` construção (id do original + campos), `07` iniciar empresa (`pessoa i32`, `nome[31]`, `cor i32`, `capital i32`, `investimento i32`), `08` financeiro (id do original + campos), `09` resposta de escolha (`id u32`, `botão u8`), `0a` conjunto de correções (`bits u64`), `0b` jogador saiu (`id u32`; é o comando `0x01` do original), `0c` jogador entrou (`id u32`, `identidade u32`, `pessoa i32`, `nome[31]`). Construção e financeiro usam os ids de pacote do original como segundo byte (`0x0a`, `0x16`..`0x21`, `0x27`, `0x29`, `0x2a`, `0x2c`, `0x2d`, `0x2f`; `0x0e`..`0x15`, `0x22`..`0x25`, `0x2e`) seguidos dos campos de `BuildCmd`/`FinCmd` em ordem; `wire.rs` é a definição.

Regras do lote (`multiplayer.md` 1.5):

- O anfitrião emite um lote quando a lista de saída não está vazia ou passou 1 s desde o anterior. `seq` = 1, 2, 3...; `stamp` = horizonte do lote anterior (0 no primeiro); `horizonte = max(anterior, T + minutos(2000 ms) + 1)`.
- Todo lote vai a todos os jogadores ativos e ao próprio anfitrião, que o aplica pelo mesmo caminho.
- Aplicação em ordem de `seq` (o lote `n` só é aplicado depois do `n-1`; os que chegam antes esperam). `SetSpeed`/`TogglePause` executam na chegada, os outros no minuto `stamp`, em ordem do lote. Nenhuma máquina passa do horizonte recebido.
- O comando `NetPlayerLeft` sai do anfitrião (ou do sucessor) como qualquer comando.

### 2.3 Snapshot

`tipo u8` (0 `Start`, 1 `Join`, 2 `Resync`), depois:

- **estado**: `arquivo bytes` (`World::to_file_bytes`, o `.GAM` com o trailer do conjunto de correções; vazio para um mundo sem pools), `minutos i32`, `velocidade u8`, `velocidade_retomar u8`, `game_seed u32`, `rng u32`, `crt u32`, `hash_dos_comandos u64`, `n u32` × (`id u32`, `identidade u32`, `nome str`) do roster, `n u32` × (`pessoa i32`, `estado u8`, `outro_venceu bool`) dos resultados;
- jogadores (como acima), `id_do_anfitrião u32`, `seu_id u32`, `último_seq u32`, `horizonte i32`, `horizonte_anterior_do_anfitrião i32`;
- comandos pendentes: `n u32` × (`stamp i32`, `remetente u32`, comando).

O receptor espera o lote `último_seq + 1`. `Start` começa a partida, `Join` entra numa em andamento (o anfitrião já passou a mandar os lotes; os que chegarem antes do instantâneo ficam guardados), `Resync` substitui o estado de uma máquina que divergiu. **Sem compressão** (o original comprime com compressor próprio; o clone deixa isso para depois).

### 2.4 Entrar no meio da partida e reconectar

Extra do clone (no original, `multiplayer.md` 5.2, não existe). Depois do `Welcome` (`em_andamento = true`) o anfitrião manda o `Snapshot` `Join` e coloca no próximo lote o comando `jogador entrou (id, identidade, pessoa)`, com a **pessoa escolhida pelo anfitrião** (`World::net_free_seat`: uma pessoa que um humano já controlou e que é IA hoje, a de mesma identidade primeiro). Assim o roster muda no mesmo minuto em todas as máquinas e nenhuma decide sozinha. Quem volta (mesma `identidade`) reassume a mesma empresa com um id de rede novo. Sem assento livre: `Reject("no free seat in this game")`. `Config::join_in_progress = false` desliga.

## 3. Sessão

### 3.1 Silêncio, batimento e desconexão

Como `multiplayer.md` 5.1, com os mesmos valores (todos em `Config`): `Heartbeat` depois de 10 s sem enviar a um par, `Probe` depois de 23 s sem ouvi-lo (repetida a cada 5 s), remoção depois de 90,001 s. O anfitrião vigia todos, o convidado só o anfitrião (as ligações entre convidados existem para a migração). `Ping` a cada 5 s mede o tempo de ida e volta por jogador (`Session::ping_ms`). Uma ligação fechada (`Leave`, `Closed`) vale na hora.

Remoção: o jogador sai do roster de conexão e o mundo recebe a remoção:
- eram 2 jogadores: `World::net_player_left` direto na máquina que fica (e o jogo vira local);
- eram 3 ou mais: o anfitrião emite o comando `jogador saiu` no lote, para cair no mesmo minuto em todos.

**Migração**: se saiu o anfitrião, cada convidado escolhe o menor id restante. O sucessor passa a emitir lotes (continua o `seq`, o `stamp` é o horizonte que viu, horizonte com +8 s de folga, `takeover`), zera os relógios de silêncio dos outros e manda `HostChanged`.

### 3.2 Checksum e dessincronização

No fim do dia 1 de cada mês (`net_time_sync` `0x49e0b0`) cada máquina guarda um ponto de verificação (digest e, em memória, uma cópia canônica do estado, as três últimas) e manda `Sync` ao anfitrião. O anfitrião compara os digests do mesmo minuto. Diferença:

1. `Event::Desync { minuto, jogador, grupos }` (os grupos são os do `Digest::differs`: `rng`, `companies`, `persons`, `trains`, `industries`, `stations`, `other`, `clock/roster`);
2. `Desync` ao jogador, que responde `StateDump` com o save canônico do ponto;
3. o anfitrião calcula o diff campo a campo (o de `rt2-savediff`) e grava `<dump_dir>/desync-<minuto>-p<id>.txt` (primeira linha: minuto e grupos; depois uma linha `caminho: nosso | deles` por campo), emitindo `Event::DesyncDump`;
4. se `resync_on_desync` (padrão), manda o próprio estado (`Snapshot` `Resync`) ao jogador.

Fora do digest (por máquina ou dependentes do tamanho do passo): velocidade, CRT, histórico anual da interface, empresa local do cabeçalho, identidade `+0x54d`, bit de luz dos tiles e ordem da lista de animações (`rt2_sim::net`).

### 3.3 Chat

`Chat` (não confiável) a todos; texto cortado em 900 caracteres; `Event::Chat { de, nome, texto }`. A interface (6 linhas, 14 s) é da fase 2.

## 4. Testes

- CI (rápidos, sem os dados do jogo): `crates/rt2-net/tests/lockstep.rs` (2 e 3 instâncias na memória com checksums iguais, atraso/jitter/perda/duplicação, horizonte, pausa de 1 min/s, chat, recusa por hash), `faults.rs` (dessincronização com dump e `Resync`, jogador que some e que sai, queda e silêncio do anfitrião com migração, último humano, entrar no meio, reconexão, recusa), `tcp.rs` (2 e 3 instâncias em sockets reais em localhost) e os testes unitários de `wire`, `msg`, `link`, `mem` e `tcp`. Todos juntos rodam em menos de 2 s.
- Locais (precisam de `RT2_DIR`): `crates/rt2-net/tests/real_game.rs` (o Example, com 9 trens, e o Alaska; 2 humanos; 14 meses; atraso e perda; entrada e saída) e `crates/rt2-sim/tests/net_mode.rs` (independência do tamanho do passo, passos de trem diários, humano vira IA, instantâneo).
