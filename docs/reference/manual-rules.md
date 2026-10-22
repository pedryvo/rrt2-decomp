# Regras, fórmulas e números de Railroad Tycoon II (referência para a reimplementação)

Documento de referência extraído dos manuais e guias originais. Serve de base para reimplementar a simulação do clone em Rust.

## Como ler este documento

- Cada item traz a fonte entre colchetes, no formato `[SIGLA > seção]`.
- Os números são citados como aparecem na fonte. Quando a fonte não dá um valor, o item está marcado como **vago** ou **não documentado**.
- Quando duas fontes divergem, o item registra as duas versões e marca **CONFLITO**. A seção "Conflitos e lacunas" lista todos.
- Texto de sabor, biografias e histórias de cenário foram omitidos, salvo quando trazem um número de regra.

### Siglas das fontes

| Sigla | Arquivo (somente leitura, em `/home/pedryvo/Games/railroad-tycoon-2/`) | Observação |
|---|---|---|
| M | `Manual.html` | Manual do usuário (RT2 Platinum). Capítulos 1 a 10 e cap. "The Second Century" (TSC). |
| SG | `Strategy Guide.html` | Guia oficial de estratégia. Capítulos 1 a 12. Contém a maior parte das fórmulas de economia e finanças. |
| AG | `Appendix - Strategy Guide.html` | Apêndice do guia: Tabela A-1 (locomotivas), A-2 (calculadora de frete), A-3 (demanda), Apêndice B (efeitos). |
| E3 | `editor3.txt` | Manual do editor de eventos ("Event Editor Manual"). Contém as listas completas de funções, efeitos e tipos de carga. |
| E2 | `EDITOR2.TXT` | Notas sobre as novas funcionalidades do editor (TSC). |
| RM | `readme3.txt` | Readme da versão 1.55. |
| HS | `HISTORY2.TXT` | Histórico de patches (versão 1.56). |

Arquivos de apoio gerados na pasta de rascunhos: `/tmp/claude-1000/-home-pedryvo-Decomp-railroad/6d7cf073-1b4e-458b-98c6-4163984526ea/scratchpad/manuals/` (texto convertido e tabelas extraídas dos HTML).

---

## Sumário

1. Tempo e calendário
2. Terreno e custo de construção (inclinação, pontes, relevo)
3. Trilhos, estações e sinalização
4. Trens: locomotivas, velocidade, peso, quebras, manutenção, composição e rotas
5. Carga: tipos, demanda, preço e receita
6. Cidades e indústrias: produção, conversões, crescimento e compra
7. Economia: estados, efeitos e variáveis
8. Finanças: empresa, ações, bônus, rating, falência e patrimônio pessoal
9. IA, níveis de dificuldade e modelos de jogo
10. Eventos e condições de vitória
11. Editor de mapas e cenários
12. Interface: telas e atalhos
13. Conflitos entre fontes e lacunas
Anexos A a Q: tabelas (locomotivas, cargas, indústrias, gerentes, chairmen, melhorias, pontes, rating, cenários, funções e efeitos do editor)

---

## 1. Tempo e calendário

- Período do jogo: 1804 a 2020 no RT2 base; no TSC, 1930 a 2030 "and beyond" [M > About Railroad Tycoon II; M > Changes/TSC What's New].
- Escala de tempo: "Trains travel about as far in one game year as they would in one day of real life" [M > Cap. 4 > Date].
- Reparo: "when your broken-down train takes 6 game months to fix" (exemplo de escala; texto, sem regra numérica) [M > Cap. 4 > Date].
- Velocidade de jogo: escala de "3" a "4" recomendada; teclas + e - aumentam e diminuem, PAUSE pausa [M > Cap. 2 > Exploring the Main Interface; M > Cap. 4 > Control Panel > General]. Velocidade "8 ou mais" dificulta ver a direção do rio no editor [SG > Cap. 9 > Adding Rivers].
- Dividendos pagos trimestralmente: "at the end of each quarter, March, June, September and December" [M > Cap. 4 > Date]. SG: pagos "in the next month after the quarter is over" [SG > Cap. 9 > Manipulating the Dividend].
- Relatório anual (annual report) aparece como pop-up; a opção "News Frequency" reduz os pop-ups de relatório anual e anúncios econômicos. Momento exato de geração não documentado [M > Cap. 4 > Control Panel > Game Play].
- Os totais de receita de estação "From/To" são recalculados a cada ano [SG > Cap. 5 > To and From Coins].
- Variável de evento `GameYearsElapsedSinceStart`: "For the first 12 months, this evaluates to 0, for the next 12 months, to 1" [E3 > Appendix A].
- Formato de data numérico `GameYearMonth`: julho de 1865 vira 186507 [E3 > Appendix A].
- Disponibilidade de motores e prédios depende da data (por exemplo, "Nuclear Plant ... not available until 1950") [M > Cap. 5 e Apêndice > Industries].
- Mudança de estado econômico: "a level every 6 to 18 months", ciclo completo "in about 5 years" [SG > Cap. 2 > Economic Status]. Ver seção 7.
- Ano inicial do mapa: campo "Start Year" ou faixa "Minimum Start Year" a "Maximum Start Year" no editor [M > Cap. 10 > Map Options > General].
- Eventos de frequência: anual ou mensal, avaliados no fim ou no início do período. No início do ano, as variáveis "year to date" são zeradas [E3 > Frequency].

## 2. Terreno e custo de construção

### 2.1 Custo base de trilho

| Tipo (por quadrado) | Não eletrificado | Eletrificado | Fonte |
|---|---|---|---|
| Single (via simples) | $4.000 | $10.000 | [SG > Cap. 5 > Table 5-4] |
| Double (via dupla) | $6.000 | $15.000 | [SG > Cap. 5 > Table 5-4] |

- "your track cost will rarely fit this table" (texto; os fatores abaixo alteram o custo) [SG > Cap. 5 > Laying Track].
- Terreno: areia (praia e deserto) **+25%**; pântano e montanha **+50%**; neve **dobra** (x2); cada árvore a cortar **+$1.000** [SG > Cap. 5 > Laying Track > Terrain].
- Grade: "Low grades do not significantly change the cost of track, but steeper grades can result in significantly higher cost" (sem números) [SG > Cap. 5 > Laying Track > Grades].
- Terraplanagem: ao construir, o terreno é nivelado "perpendicular to the direction the rails run" (custo alto em encostas) [SG > Cap. 5 > Terraforming].
- Manual, custo de ponte: "can be as much as ten times as expensive as regular track" [M > Cap. 4 > Lay Track > Bridge Type].
- Custo que aumenta com "steep grades, heavy forests, deserts and other hostile terrain" (sem números) [M > Cap. 4 > Lay Track > Cost].
- Custo de trilho por economia: "+10% per economic level" acima ou abaixo do normal [SG > Cap. 2 > Other variables].
- Upgrade: custo depende do tipo antigo e do novo. Downgrade não é permitido. Single para double electrified custa o mesmo que construir double electrified novo, "starts at a base of $15" (provável erro de unidade; ver Tabela 9) [SG > Cap. 5 > Upgrading].
- Upgrade de via simples para eletrificada: a ponte deve ser atualizada "at no cost" para trens elétricos passarem [SG > Cap. 5 > Table 5-3 nota].
- Manutenção de trilho: "Track costs money to maintain" (sem valor) [SG > Cap. 5 > Routes].
- Trilho de outra empresa: pagamento proporcional ao trecho percorrido, ver seção 8 (Finanças e receita).

### 2.2 Grades (inclinação)

- Cores das grades (SG): verde 0 a 1,9%; amarelo 2% a 3,9%; vermelho 4% e acima [SG > Cap. 5 > Laying Track].
- Manual: "A grade greater than 4.0 is considered steep, and should only occur if the obstacle is unavoidable. Anything higher than 4.0 will kill most routes, in the early 1800's" [M > Cap. 4 > Lay Track > Grade]. CONFLITO parcial: manual trata 4,0 como o limite de "íngreme"; SG usa 4% para vermelho.
- Manual: "Yellow numbers are steep (red are the steepest), which will slow your trains down" [M > Cap. 2 > Laying Your First Track].
- Efeito da grade no peso: "For every 2 percent grade climbed, weight is roughly quadrupled. For example, a train hauling 120 tons up a 2% grade is effectively hauling about 480 tons" [SG > Cap. 4 > Breakdowns and Crashes]. Ver seção 4.
- Efeito da grade sobre a areia: sem areia a grade é "doubled, so a 2% grade acts like a 4% grade" (SG). O manual diz "its ability to climb grades is halved (a 2% grade acts like a 4% grade)" [M > Cap. 6 > Instrument Panel > Sand]. Consistente.
- Terraplanagem para reduzir grade: "reduction of a terrible slope (10 percent grades and more) to around 1 percent or 2 percent" (SG, técnica, sem fórmula) [SG > Cap. 5 > Routes].
- Trem sozinho sobe a grade com melhor desempenho; "the engine and loaded coal cars will shoot down the grade at close to top speed" (texto) [SG > Cap. 5 > Routes > General Tips].
- Curvas: trens desaceleram em curvas (sem números) [SG > Cap. 5 > Routes].

### 2.3 Pontes

| Tipo | Ano disponível | Custo (terreno plano) | Restrições | Fonte |
|---|---|---|---|---|
| Madeira (Wooden) | 1830 | $60.000 | Só via simples. "trains slow down 40 percent while crossing" | [SG > Cap. 5 > Table 5-3] |
| Pedra (Stone) | 1840 | $200.000 simples / $300.000 dupla | Nenhuma | [SG > Cap. 5 > Table 5-3] |
| Aço (Steel) | 1880 | $150.000 simples / $225.000 dupla | "Trains slow down 20 percent while crossing" | [SG > Cap. 5 > Table 5-3] |

- Manual: "Wooden bridges are cheap to build, but can only support single wide track, and force trains to slow down greatly" [M > Cap. 4 > Lay Track > Bridge Type].
- Rios: há apenas um tipo de rio; "Crossing large bodies of water is impossible, so rivers are your only solution for adding water that can be bridged by players" [SG > Cap. 9 > Adding Rivers]. Só rios podem ser pontilhados; água sem rio não pode ser atravessada [M > Cap. 10 > Lay River].
- Vãos sobre água aberta (spanning bridges, TSC): reta, somente via simples, não pode ser dupla [RM > (3) A]; custo: **$120.000 por seção** [RM > (3) A] vs **$150k por seção** [SG > Cap. 11 > New Functions]. CONFLITO. SG também diz "$2-3 million dollar bridges are quite common".
- Terreno: não se pode colocar trilho sobre oceano nem através de prédio existente [M > Cap. 4 > Lay Track; SG > Cap. 5 > Laying Track].
- Interseções: só em ângulos de 45° e 135°; 90° é ajustado automaticamente; cruzamento em ângulo reto não conta como conexão e trens não viram ali [SG > Cap. 5 > Laying Track].
- Túneis: nenhum texto sobre túneis nas fontes (não documentado). A palavra aparece só em biografia de pessoa, sem regra.

### 2.4 Mapa, relevo e terreno (editor)

- Ferramentas de terreno: Raise/Lower Ground (com três opções: subir/descer, suavizar, nivelar ao nível do mar) e pincel de cinco tamanhos [M > Cap. 10 > Raise/Lower Ground].
- Smooth Out: "SHIFT ... will level the area to the center point" [SG > Cap. 9 > Tools].
- Fill Terrain: All/Adjacent; Match/Above/Below [M > Cap. 10 > Fill Terrain].
- Água: "The water tile is the only tile that will modify the terrain by leveling" [SG > Cap. 9 > Adding Lakes].
- Imagem importada: .pcx em escala de cinza indexada; branco = mais alto, preto = mais baixo; slider de suavização, "100 to 150 usually gives you the best results" [SG > Cap. 9 > Starting from an image].
- Tamanho máximo de mapa sem mais de 16 MB de RAM: 125.000 quadrados (500 x 250) [M > Cap. 10 > Importing an Image].
- Tipos de terreno (tiles) e efeitos sobre custo: apenas os fatores da seção 2.1.

---

## 3. Trilhos, estações e sinalização

### 3.1 Tipos de via

- Single (via simples): um trem por trecho; trens esperam. Quem tem a prioridade decide; "Lower priority trains always stop for higher priority trains. If two trains of the same priority meet, whichever train entered the grid square first wins" [SG > Cap. 5 > Track Types > Single track].
- Trens de concorrentes em sua via têm sempre prioridade menor; trens seus em via alheia também têm prioridade menor que os da própria empresa [SG > Cap. 5 > Track Types].
- Double (via dupla): trens mantêm o lado direito, não se encontram de frente; "Three or more trains are almost always too much for a single stretch of track" [SG > Cap. 5 > Track Types > Double track]. Manual: "Double track is a must for busy routes" [M > Cap. 4 > Lay Track].
- Eletrificada: custo maior; trem elétrico exige rota inteira eletrificada, senão fica parado na estação com a mensagem "No Electricity" [M > Cap. 6 > More On Train Routing; SG > Cap. 5 > Track Types > Electric track]. Vapor e diesel rodam em trilho eletrificado normalmente [SG > Cap. 5 > Track Types].
- Pontes: madeira, pedra, aço (ver seção 2.3).

### 3.2 Estações

| Tamanho | Custo | Área de captação | Fonte |
|---|---|---|---|
| Small (pequena) | $50.000 | Quadrado de 5 x 5 | [M > Cap. 4 > Build a New Station; SG > Cap. 5 > Size Does Matter] |
| Medium (média) | $100.000 | Círculo de diâmetro 9 ("2,9 vezes a área" da pequena, texto) | [M > Cap. 4; SG > Cap. 5 > Size Does Matter] |
| Large (grande) | $200.000 | Círculo de diâmetro 17 ("3,5 vezes a média e 10,2 vezes a pequena", texto) | [M > Cap. 4; SG > Cap. 5 > Size Does Matter] |

- A área de captação é determinada "by the track in front of the station, not the station itself"; orientação e lado da via não mudam a área [SG > Cap. 5 > Other Placement Considerations].
- Estação com sobreposição de área com outra estação: a carga é dividida "on a first-come, first-served basis" (quem chega primeiro leva a carga) [SG > Cap. 1 > Building Stations; SG > Cap. 5 > Other Placement].
- Cores de colocação: verde = válida e conectada; amarela = válida, sem conexão; vermelha = inválida (água, sobreposição com prédio) [M > Cap. 4 > Build a New Station].
- Orientação: centro = automática; 8 caixas = orientação forçada [M > Cap. 4 > Build a New Station].
- Arquitetura (Default, Victorian, Colonial, Mission Revival, Tudor, Kyoto Revival): "visual only" [M > Cap. 4; SG > Cap. 5 > Different Types and Flavors].
- Trem que entra e sai em linha reta tem turnaround mais rápido [SG > Cap. 5 > Other Placement].
- Estação pequena pode ser depósito de serviço ao longo de rota longa [SG > Cap. 5 > Stations from the Inside > TIP].
- Trens precisam **parar** na estação para carregar/descarregar, mas só precisam **passar** por ela para reabastecer areia, água e óleo [M > Cap. 5 > Servicing Trains; SG > Cap. 5 > TIP].
- Atraso de coleta: a carga disponível se deteriora com o tempo; melhorias de armazenamento reduzem a penalidade em 50% (ver 3.3).
- Demanda de cargas não atendidas: "Cargo which is demanded at a station is always sold upon a train's arrival there" [RM > (3) B].
- Upgrade de tamanho (pequena para média, média para grande): custa o mesmo que a estação original seria ao ser construída [SG > Cap. 5 > Stations from the Inside].

### 3.3 Melhorias de estação (tabela completa)

Ver Anexo I no fim do documento (Tabela 5-1 do SG). Resumo das regras:

- Turnaround: Telegraph Poles (1837, $40.000, -25%) substituído por Telephone Poles (1905, $80.000, -50%). Customs House (1830, $25.000): reduz atraso de alfândega em 50%, só para trem que entra em outro território ou porto [SG > Cap. 5 > Stations from the Inside].
- Receita de passageiros: Saloon +10% ($16.000, 1830); Restaurant pequeno +20% ($50.000, 1830); Restaurant grande +30% ($100.000, "late 1850"); Hotel pequeno +20% ($65.000, 1830); Hotel grande +40% ($175.000, "late 1878") [SG > Cap. 5 > Stations from the Inside].
- Atraso de coleta (penalidade de pickup) -50%: Warehouse (1830, $25.000: goods, cotton, wool); Post Office (1840, $35.000: mail); Refrigerated Storage (1860, $50.000: produce, milk); Grain Silo (1873, $35.000: grain, coffee, sugar); Liquid Storage (1885, $50.000: oil, chemicals) [SG > Cap. 5 > Stations from the Inside; SG > Cap. 5 > Table 5-1].
- Serviço ao trem (disponíveis desde o início): Water Tower ($25.000; só vapor), Sanding Tower ($30.000), Roundhouse ($120.000; óleo/manutenção) [SG > Cap. 5 > Stations from the Inside; M > Cap. 5 > Servicing Trains].
- Efeito de falta: sem água, o vapor "travel much slower" (SG); sem areia, "Trains without sand travel 50% slower" (tabela, SG) e "run twice as slow" (texto, SG). CONFLITO (50% mais lento vs. duas vezes mais lento). Sem óleo: risco de quebra cresce, ver seção 4.
- Intervalos recomendados (texto, não são regra do jogo): água a cada ~30 quadrados; areia a cada 10 em terreno montanhoso e 50 a 100 em plano; roundhouse "100 squares or more between visits" [SG > Cap. 4 > Stations].
- Visitas de roundhouse: "usually once every two to three years" (texto) [M > Cap. 5 > Servicing Trains].
- Em RT2 base, estações não guardam carga de outras estações: "You can't haul a cargo to one station, drop it off, and then haul it to the next station" [SG > Cap. 1 > Train/Consists Management]. O armazenamento é recurso TSC (3.4).

### 3.4 Destino da carga não demandada (TSC)

Cada parada da rota tem uma bandeira (depot flag) que define o que fazer com carga que a estação não demanda. Carga demandada é sempre vendida na chegada [RM > (3) B].

| Bandeira | Comportamento | Fonte |
|---|---|---|
| Default | Se a carga é demandada em parada posterior, fica no trem; senão é entregue aqui. Igual ao RT2 original. | [M > Cap. 5/TSC > Interface Changes; RM > (3) B] |
| Store (armazenar) | Fica armazenada na estação; qualquer trem pode pegar, inclusive o que deixou. | [M > TSC > Cargo Depoting] |
| Deliver (entregar) | Entregue aqui; "You won't receive much money, though." | [M > TSC > Cargo Depoting] |
| Leave on train (deixar no trem) | Fica no trem indefinidamente se a bandeira for esta em todas as paradas. | [M > TSC > Cargo Depoting] |

- Carga não demandada, preço de venda: ver seção 5.

### 3.5 Waypoints (TSC)

- Ctrl + arrastar no trilho adiciona um ponto de passagem sem parar [M > TSC > Route Waypoints; SG > Cap. 11].

### 3.6 Sinalização e prioridades

- Não há sinais de via (semáforo de linha) nas fontes. O "traffic signal" documentado é o semáforo de carga de cada parada (ver abaixo) [M > Cap. 6 > Route/Consist List Box].
- Classe do trem (ícone na Lista Central): **Express** passa qualquer trem que não seja Express; **Normal** cede a Express; **Slow** cede a Normal (indicado para carga pesada, como carvão); **Halted** para o trem até ser liberado [M > Cap. 4 > Center List Box > Trains].
- Semáforo de carga na lista de rota (cada parada): **Verde** = não espera, pega o que estiver disponível; **Amarelo** = espera até pelo menos metade (arredondada para cima) dos vagões de carga estarem cheios; **Vermelho** = espera até todos estarem cheios [M > Cap. 6 > Route/Consist List Box; M > Cap. 2 > Selecting Destinations and Cars].
- Manual: "Be careful with this option, if you overuse it, your trains spend most of their time waiting around in stations" [M > Cap. 6].
- Congestionamento: "if a train spends more than ten percent of its time between stations yielded to other ..." (texto truncado no original) [SG > Cap. 5 > Double track].

---

## 4. Trens

### 4.1 Tipos de locomotiva e atributos

- Três famílias: vapor (steam), diesel e elétrica (electric). Vapor queima carvão, madeira ou óleo [M > Cap. 6 > Locomotives].
- Diesel: "diesel-electric"; elétrica: exige eletrificação de toda a rota [M > Cap. 6 > Locomotives; M > Cap. 6 > More On Train Routing].
- Atributos mostrados na compra: Cost, Maintenance (estimativa anual), Fuel (estimativa anual, depende da distância percorrida), Acceleration, Reliability, matriz de velocidade máxima (toneladas x grade) [M > Cap. 6 > Train Purchase Window; SG > Cap. 4 > Purchasing Factors].
- Velocidade de base: "All rated speeds for an engine are based on this throttle setting" (throttle 85%) [M > Cap. 6 > Instrument Panel > Throttle].
- A matriz de velocidade usa carga "#@%" = toneladas @ grade em %. Os valores da Tabela A-1 são "base level" e podem ser modificados por data, cenário, gerente e mapa [AG > Table A-1].
- Peso por vagão ajustável no painel de compra para simular carga pesada (carvão) ou leve (passageiros) [M > Cap. 6 > Train Purchase Window]. SG: vagões de 40 toneladas na matriz padrão [SG > Cap. 4 > Purchasing Factors > Top Speed].
- Disponibilidade por região: colunas A (norte-americana), E (europeia), W (mundial), e anos First (primeiro ano à venda) e Last (último) [AG > Table A-1].
- Engine tem "Reliability" (confiabilidade) de categoria; a escala e o alcance de quebra estão em 4.4.

#### Tabela A-1 (Apêndice do guia, "Locomotive Data")

Ver Anexo A no fim do documento.

#### Tabela 4-1 do manual ("The Iron Horses at a glance")

Ver Anexo B no fim do documento. Esta tabela traz tipo (Steam/Electric/Diesel) e custo inicial, manutenção e combustível anuais. Os valores de algumas células foram transcritos com erros de digitação do original ("284,00", "265,00", "1,000 K", "2,500K"); manter como estão e corrigir na implementação após checagem.

### 4.2 Velocidade e tração

- Throttle padrão: 85% do máximo; "the highest value that is reasonably safe"; a zona vermelha dá força extra com risco maior [M > Cap. 6 > Instrument Panel > Throttle].
- Throttle totalmente aberto: chance de quebra "four times normal" (M; SG: "quadruples"). Throttle a metade: "reduces breakdown chances for the equivalent distance traveled by half" (M).
- Envelhecimento: perde "about a half percent off their top speed every year" (M). "A 20-year-old engine is about 10 percent slower than a new engine of the same type" (M).
- Sem água (só vapor): velocidade "greatly reduced", mas sem efeito até zerar (M; SG: "There is no adverse effect until the train runs completely out of water").
- Sem areia: sem efeito até zerar; depois a capacidade de subir grade cai à metade ("a 2% grade acts like a 4% grade") (M; SG concorda).
- Sem óleo: chance de quebra e custo de manutenção sobem (ver 4.4 e 4.5).
- Acelerações: classificação qualitativa (SG). Sem fórmula de aceleração documentada.
- Trem sozinho: "Most trains ... can run up a mountain at a decent speed as long as it is not hauling any freight cars" (texto) [SG > Cap. 5 > Routes > General Tips].
- Tabela de velocidade (A-1) para grades: colunas 0@0%, 150@0%, 300@0%, 150@4%, 300@4% (toneladas / grade), unidade de velocidade não rotulada no guia (vago).

### 4.3 Peso e grade

- "The first 50 tons hauled are free. Every additional 200 tons hauled doubles the breakdown odds" [SG > Cap. 4 > Breakdowns and Crashes > Weight Hauled].
- Grade: "For every 2 percent grade climbed, weight is roughly quadrupled" (efeito de peso efetivo, exemplo: 120 t a 2% = ~480 t) [SG > Cap. 4 > Breakdowns and Crashes > Grade].
- Trens antes de 1850: manter leves; "especially true of trains before 1850" [SG > Cap. 4 > Cargo].
- Mais de 4 vagões carregados é preferível para lucro: "preferably 4 or more loaded cars" (texto, TSC) [M > TSC > Strategy Tips].
- Pré-1880: "keep trains below three cars initially" (texto) [SG > Cap. 1 > Train/Consists Management].
- Cenários específicos recomendam limites (John Bull com 2 vagões; Shinkansen com até 3): texto de dica de cenário, não é regra geral [SG > Cap. 6].

### 4.4 Quebras e acidentes

Chance base: função da confiabilidade ("Cells Traveled Between Breakdowns (Average)"). Quebras são baseadas em distância percorrida, não em tempo [M > Cap. 6 > Train/Station Info; SG > Cap. 4 > Table 4-1].

| Confiabilidade (rating) | Células entre quebras (média) |
|---|---|
| Atrocious | 400 |
| Extremely Poor | 600 |
| Very Poor | 875 |
| Poor | 1.100 |
| Below Average | 1.400 |
| Average | 1.800 |
| Above Average | 2.500 |
| Good | 4.000 |
| Very Good | 6.000 |
| Outstanding | 12.000 |
| Near Perfect | 24.000 |

[SG > Cap. 4 > Table 4-1]

- Cada nível de confiabilidade é "about a third less likely to breakdown than the one below it" (M).
- Multiplicadores de chance de quebra:
  - Óleo: vazio = **x3** ("triple"); uma marca acima do vazio = **x2**; meio cheio = **+50%** (SG).
  - Óleo: "nearly full significantly reduces breakdown risk. Running out of oil altogether ... tripling breakdown chances" (M).
  - Throttle: normal (85%) é a base; máximo = **x4** (SG). Throttle vermelho quadruplica (M).
  - Peso: 50 t livres; +200 t = **x2** (SG).
  - Grade: peso efetivo x4 a cada 2% (SG; ver 4.3).
  - Idade: "After 2 years, the odds of breakdown rise steadily. A 10-year-old train is 33 percent more likely ... A 20-year-old train is 66 percent more likely" (SG). CONFLITO: M diz "a 20-year-old engine has about a 60 percent higher risk than a 5-year-old engine" e "a new engine has virtually no breakdown risk".
  - Caboose: **-25%** de chance de quebra (SG e M).
  - Gerentes: bônus de segurança (Train Safety) (M; tabela de gerentes).
- Quebra vs. acidente: depois de sorteada a falha, "a straight 20 percent chance of it crashing and an 80 percent chance of it having a breakdown" (SG).
- Quebra: trem para por um tempo, bloqueia trens atrás, reduz goodwill (M e SG).
- Patch 1.53 (HS): após 1920, chance de quebra -15% e chance de que a quebra seja acidente -15%; após 1970, mais -15%.
- Cheat (HS): TAB e digitar `nowreck` (ou parâmetro `/nowreck`) elimina acidentes.
- Envelhecimento e manutenção: M diz "replacing engines as they reach 15 to 25 years" e SG diz "Replace trains after 10 to 20 years" (texto). Sem regra de quebra automática por idade além dos multiplicadores acima.

### 4.5 Manutenção e combustível

- Custo anual de manutenção e combustível por locomotiva: ver Tabela A-1 e 4-1.
- Idade: "A 20-year-old engine costs about three times as much in annual maintenance" (M).
- Sem óleo: "maintenance costs go up significantly" (M; sem fator).
- Combustível: "Faster trains will tend to use more fuel, since they travel further per year" (SG). Custo de combustível de vapor pode ser alterado por gerentes (ver tabela de gerentes).
- Manutenção de estradas: "Track costs money to maintain" (ver seção 2.1).
- Despesas de viagem por vagão ou carga: não documentadas além do custo anual do motor.

### 4.6 Consist (composição) e vagões

- Vagões: 32 tipos de vagão de carga e 2 especiais (vagão-restaurante "dining car" e "caboose") (M > Cap. 5 > Cargo).
- Vagão-restaurante: "increases passenger revenues by 20 percent"; funciona com 3 ou 4 vagões de passageiros (M). "Dining cars attached to at least one other passenger car increase the value of that haul by 20 percent" (SG).
- Caboose: "increases train safety (which reduces breakdown and crashes), and train security (which reduces robberies)" (M). Reduz quebras em 25% (ver 4.4).
- Carga por vagão: cada vagão carregado = uma "carga" (load) para contagem e preço (M/SG). "The number of cars supplied or demanded is indicated by the number of pictures" (SG).
- Preço de carga "stored" e deterioração: ver seção 5.
- Trens "must actually stop at a station to pick up or deliver cargo" (M).
- Manter consist ao chegar: "Don't send a train out empty" (SG, texto de estratégia).
- Vagões de comutadores (TSC): um único tipo de vagão (com faixa cinza), mas 4 tipos de demanda (tipos METRA, ver seção 10.2).

### 4.7 Rotas e roteamento

- Trem escolhe a rota mais curta entre duas estações; "Trains will generally follow the shortest route" [M > Cap. 6 > More On Train Routing].
- Uso de trilho de concorrente: paga-se ao dono "a portion of the revenue received equal to the portion of the total distance traveled which was on his track"; o dono não paga custo operacional. Exemplo: 8 de 50 casas = 20% da receita de $100.000 [M > Cap. 6 > More On Train Routing]. SG (Cap. 2, Freight Revenue 101) traz o exemplo: 16 de 20 trechos são próprios = 80% da receita não-estação ($80.000 de $100.000); a UP fica com $20.000.
- Taxa para usar estação de concorrente: M diz **$2.000 por carga** carregada ou entregue em estação de concorrente [M > Cap. 6 > More On Train Routing]. SG (Cap. 2, Freight Revenue 101) diz **$1.000 por carga** na origem não própria e **$1.000 por carga** no destino não próprio, além de perder bônus de hotel/restaurante/saloon. SG (Cap. 5, Income) diz **$2.000 por vagão** que outra empresa paga pelo uso de sua estação. CONFLITO (ver seção 13).
- Trem elétrico: toda a rota (estação a estação) deve estar eletrificada [M > Cap. 6 > More On Train Routing].
- Rotas de sistema: Main Line, Spur, Loop/Circuit, Hub, Web, Broken Line: descrições qualitativas (SG > Cap. 5 > Track Systems).

### 4.8 Roubos (Robbery)

- Nível geral de roubos (Overall Robbery Level, editor): "The Jesse James era, circa 1875, would be about 200 on this scale, while a more crime-free period would be around 30" [M > Cap. 10 > Robbers].
- Caboose reduz roubos; em área de alto crime, a caboose tem valor (SG e M).
- Gerentes com segurança (Train Safety) reduzem roubos e quebras (SG e tabela de gerentes).

### 4.9 Eventos de disponibilidade de trem

- Efeito de evento "(Locomotive Name) Availability": Normal, IMMEDIATE ou NEVER. "Any locomotive can be triggered" (E3 > Appendix B).
- Motores pioneiros também podem ser liberados cedo por bônus de cenário ("John Bull early", "Shay early", etc.) (SG > Cap. 6).

### 4.10 Visual e status da Lista de trens (interface)

- Linha de progresso verde/amarela/vermelha = velocidade relativa à máxima (verde = máxima; amarela = ~metade; vermelha = parado/lento) [M > Cap. 4 > Center List Box > Trains].
- Indicadores de falta: Oil, Water, Sand; Breakdown, Wreck, Robbed (texto na entrada) [M > Cap. 4 > Center List Box > Trains].
- Moedas douradas = valor da carga, $10.000 por moeda; o valor cai com o tempo [M > Cap. 4 > Center List Box > Trains].

---

## 5. Carga

### 5.1 Tipos de carga (produção, demanda e preço)

- O RT2 base tem 34 tipos de carga (M > Cap. 1 > Railroad Tycoon vs. Railroad Tycoon II) e 32 tipos de vagão de carga + 2 especiais (M > Cap. 5 > Cargo). No TSC há carga "Troops, Ammo, Weapons, Alcohol" e tipos METRA (E3 > Appendix A; EDITOR2).
- Tabela de demanda (Tabela 2-1 do guia, "Cargo Demand Table"). Colunas: Cargo, Base Price, Days to Deliver, Ship Distance, First Year, Cutoff for Towns, Cutoff for Cities, Demand per House, Storage Building. Tabela completa em Anexo C no fim do documento [SG > Cap. 2 > Cargo Pricing > Table 2-1].
  - "Base Price" é o preço base por carga (valor em $). "Days to Deliver" é o prazo, em dias, após o qual a carga some da estação ("if you do not pick up mail within 300 days, it will disappear from the station") [SG > Cap. 2 > Cargo Pricing].
  - "Ship Distance" é o modificador de preço por distância ("the higher this number is, the more you will receive ... across longer distances") [SG > Cap. 2 > Cargo Pricing].
  - "Demand per House": demanda por casa em cidade/vila (0 para carga sem demanda por casas). Ex.: Passengers 0.35, Mail 0.18.
  - Cutoff (for Towns/Cities): último ano em que aceitam a carga; 9999 = sem fim; 0 = não se aplica. Interpretação do valor 0 e 9999 não está explicada no guia (vago).
  - "Storage Building": melhoria da estação que reduz a penalidade de atraso desta carga.
- Tabela A-2 do apêndice (calculadora de frete): Base Value, Delivery Time Days, Rot Factor, By Towns, By Cities, Demand Modifiers. Ver Anexo D no fim do documento [AG > Table A-2].
  - "Rot Factor" (perecibilidade): "the later the date, the rot factor is increased as trains become faster" (texto) [SG > Cap. 2 > Freight Revenue 101]. Os valores são "Rot Factor*" na tabela (sem fórmula explícita).
- Valores de base do frete (SG, texto de exemplo): ferro $28.000; aço $35.000; mercadorias $42.000; total de três $105.000 [SG > Cap. 2 > Local Economy > Supply].

### 5.2 Preço da carga

- Preço = valor base x fator de distância x fator de tempo x demanda da estação (com modificadores de economia e bônus). O guia diz: "Cargo pricing starts with the base value listed in the chart. It is modified for distance hauled and time to deliver according to the factors mentioned above" [SG > Cap. 2 > Cargo Pricing].
- Fórmula exata de distância e de tempo: **não documentada** (apenas a tabela de fatores).
- Demanda da estação, Expert (Tabela 2-2 do guia): nível 0 = 50%; 1 = 60%; 2 = 70%; 3 = 80%; 4 = 90%; 5 = 100%; 6 = 110%; 7 = 120%; 8 = 130%; 9 = 140% do preço cheio [SG > Cap. 2 > Table 2-2; SG > Cap. 5 > Table 5-2]. "There's almost a threefold price swing between the highest and lowest level" [SG > Cap. 2 > Cargo Pricing].
- Demanda da estação, Appendix A-3 (Basic / Advanced / Expert): "None" = 80% / 50% / 20%; nível 0 = 100% / 100% / 50%; nível 1 = 100% / 100% / 60%; nível 2 = 100/100/70; nível 3 = 100/100/80; nível 4 = 100/100/90; nível 5 = 100/100/100; nível 6 = 100/100/110; nível 7 = 100/100/120; nível 8 = 100/100/130; nível 9 = 100/100/140 [AG > Table A-3]. Ver Anexo E.
- Carga entregue **sem demanda** na estação: SG diz "80 percent, 50 percent, and 30 percent of full price" para os modelos Easy, Medium e Hard [SG > Cap. 2 > Cargo Pricing]. AG diz "None" = 80% / 50% / 20% para Basic / Advanced / Expert [AG > Table A-3]. CONFLITO (30% vs 20% no modelo mais difícil; e a correspondência de nomes de modelo não é explícita). O manual só diz "a decent price, not quite as much as if the station demanded it" (Easy), "much less" (Advanced) e "barely receive anything" (Expert) [M > Cap. 5 > The Economy].
- Modelo Basic/Easy de indústria: "a steel mill will always produce steel". Advanced/Expert: indústria só produz se suprida; "Advanced: Stations still accept any cargo, but at half price if the cargo isn't demanded" (M > Cap. 3 > Options). CONFLITO de nomes (ver seção 7).
- Demanda das estações em Expert: "The higher it is, the greater the price the station will pay for that item. If a station is oversupplied with an item, the demand level will fall" [SG > Cap. 5 > Cargo Supply/Demand].
- Nível de demanda: depende de quantas indústrias demandam e de quanto da carga chegou recentemente; "If they receive a lot of cargo, demand falls off" [SG > Cap. 2 > Cargo Pricing].
- Demanda da estação em zero: "you lose half your money by supplying a station with a cargo that is at '0' demand" [SG > Cap. 5 > Interpreting Station Information].
- Bônus de receita (SG > Cap. 2 > Buying Industries > Bonuses): dining car +20%; trem com recorde de velocidade (passageiros) +15%; "boom times" +20%; restaurante grande +30%; restaurante pequeno +20%; hotel grande +40%; hotel pequeno +20%; saloon +10%.
- Mudança do preço de ação e custo por estado econômico: ver seção 7.
- Bônus de gerente: "Company managers can add a percentage of added value to certain cargo" (ex.: Carnegie: aço). Ver tabela de gerentes [SG > Cap. 2 > Bonuses; SG > Cap. 3 > The Managers].
- Eventos: "a coal strike raising the price of coal hauls" (exemplo de evento) [SG > Cap. 2 > Bonuses].
- Preço de carga: "Other factors, such as timeliness of delivery and distance hauled affect the rate as well" (ver acima) [M > Cap. 5 > Cargo].
- Velocidade: carga "mail, produce, and milk" mais sensível (M). Passageiros mais sensíveis a tempo e distância (M > Cap. 7 > Passengers and Freight).

### 5.3 Revenda entre estações e divisão de receita

- Carga de estação alheia: taxa de $1.000 por carga na origem e de $1.000 por carga no destino, mais perda de bônus de hotel/restaurante/saloon no destino [SG > Cap. 2 > Freight Revenue 101]. CONFLITO com M (ver seção 4.7).
- Exemplo de divisão: 3 cargas de carvão, 20 quadrados no total, 16 próprios e 4 da UP, receita $106.000; taxa de estações $6.000; receita não-estação $100.000; empresa fica com 16/20 = 80% ($80.000); UP com $20.000 [SG > Cap. 2 > Freight Revenue 101].

### 5.4 Demanda de passageiros e correio

- Passageiros: "Passengers are moved because they want to be. No new product is created" (texto) [SG > Cap. 7 > Passengers and Freight].
- Demanda "per house": ver tabela de cargas (Passengers 0.35; Mail 0.18).

---

## 6. Cidades e indústrias

### 6.1 Produção de indústrias

- Taxa base: mina de ferro produz "two loads of cargo per year" (SG > Cap. 2 > Supply). Fazendas e pomares (algodão, grão, açúcar, produce, café): base de "two cargoes per year", com **+50%** se abastecidos com fertilizante; "One load of fertilizer is used for every two loads of crops produced" (SG). Pecuária (gado, ovelha, leite): **+50%** com grão (SG).
- Indústrias que só produzem com insumo: aço (ferro + carvão). "Given one load each of iron and coal, you can generate up to six haulable loads, for three times normal profits" (SG).
- Tabela de produção e compra (Apêndice, "The Industries"): colunas Purchase $, Demands, Produces, Qty. Annually, Bonuses. Tabela completa em Anexo F no fim do documento [M > Apêndice > The Industries].
- Casas (Housing): "Passengers/Mail 0.5/0.2" por ano (coluna de produção) [M > Apêndice > The Industries > Housing].
- "N/A" na coluna de quantidade anual significa que a produção depende do insumo (M).
- Portos: até 4 conjuntos de demanda/produção definidos no editor; alfândega ("customs house") é necessária perto de portos (M > Apêndice > Port).
- Indústria de energia elétrica (Electric Plant) e nuclear produzem energia (não transportável) e resíduos (M > Apêndice).

### 6.2 Conversões (cadeias de produção)

- Exemplo de cadeia (M e SG): ovelha -> lã -> fábrica têxtil -> mercadorias -> cidade (M > Cap. 5 > The Economy); "Coal -> Steel Mill -> Steel -> Tool and Die Factory" (SG > Cap. 1 > Expansion).
- Lista completa de conversões: Anexo F.

### 6.3 Cidades, vilas e casas

- Demanda de estação: soma das demandas de indústrias e casas dentro do raio [M > Cap. 5 > The Economy; SG > Cap. 2 > Demand].
- Town: estação com **4 ou mais** casas no raio demanda alimentos, mercadorias, madeira e leite [M > Cap. 5 > The Economy].
- City: **8 ou mais** casas demandam tudo que a cidade demanda, mais autos, cimento, papel, óleo e carvão (M). SG diz "everything a town demands, plus autos, paper, and other products" [SG > Cap. 5 > Cargo Supply/Demand].
- Tamanho populacional (SG): Village 1 a 2 prédios provável; Town 1 a 4; City 4 a 8; Metropolis 6 a 11; Megalopolis 8 ou mais. Anexo M.
- Casas não podem ser compradas como indústria (M e SG).
- Crescimento: cidade bem servida pode chegar a "roughly 3 times its original size" em 10 a 15 anos; cidade sem serviço "will often shrink by 10 to 30 percent over the course of a decade or two" (SG > Cap. 2 > City Size and Growth).
- Tamanho inicial: "random factor ... accounts for about 30 percent of a city's starting size" (SG > Cap. 2 > City Size and Growth).
- Regiões: "Most maps have 10 to 20 regions defined" (texto) [SG > Cap. 2 > City Size and Growth].
- Cidade Detroit (exemplo): "about a 50/50 shot of having a lumber mill" (exemplo de aleatoriedade, texto).
- Taxa de crescimento das regiões: "more moderate" (texto).
- Indústrias "can survive without your rail service, but if you do a good job within a particular area, you'll really see it thrive" (texto).
- Ano de disponibilidade das indústrias (ver Apêndice): Paper Mill 1800; Produce Orchard 1800 (para cidades até 1876, depois via cannery); Sheep Farm, Sugar Farm, Textile Mill, Tool and Die Factory e Cotton Farm (sem ano, "one of the earliest"); Dairy Processor "around 1880"; Electric Plant (não aparece antes de 1890); Gravel Pit 1884; Fertilizer Plant 1905; Rubber Farm 1900; Tire Factory 1900; Nuclear Plant 1950; Uranium Mine 1950 (M > Apêndice > The Industries).
- Café e produce: "after 1876" o café tem de passar por cannery (M > Apêndice > Coffee Farm); towns e cities aceitam café e produce direto até 1878 (RM > (3) C).
- Oil Well: "directly to cities after 1890" (M).

### 6.4 Compra de indústrias (apenas modelo Expert, segundo o SG)

- Preço de compra: ver tabela de indústrias (aciaria $800.000; fazenda de algodão $200.000) [M > Apêndice > The Industries; SG > Cap. 2 > Buying Industries].
- Lucratividade (rating): "Gushing Cash" até "Poor" (M > Cap. 4 > Viewing Buildings). Só comprar "lucrative", "very lucrative" ou "gushing cash" (SG).
- CONFLITO sobre o modelo: SG diz que a compra só existe no modelo Expert; M (Cap. 4 > Viewing Buildings) diz que a opção de compra aparece com o modelo industrial Advanced. Ver seção 13.
- Retorno: "An industry rated as 'gushing cash' will usually make an annual return of 20 percent or more on its purchase price" (SG).
- Lucratividade é uma média móvel de alguns anos; para contar, a carga apanhada deve ser entregue a estação que a demanda [SG > Cap. 2 > Buying Industries].
- Regra geral: distribuir a carga entre 3 ou mais indústrias demandantes mantém o preço alto [SG > Cap. 2 > Buying Industries].
- "Industry Buying" e "Price Levels": ver Expert em seção 7.

---

## 7. Economia

### 7.1 Estado econômico do mapa

- Cinco estados: Depression, Recession, Normal, Prosperity, Boom Times [SG > Cap. 2 > Economic Status].
- Variável `GameEconomicState`: 0 (depressão) a 4 (boom) [E3 > Appendix A].
- Efeito de evento `Economic Status (instant)`: -4 a +4 níveis ("4 being the best, -4 the worst") [E3 > Appendix B].
- Muda um nível a cada "6 to 18 months", com ciclo completo "in about 5 years"; tem inércia ("momentum"): após subir, é mais provável subir de novo [SG > Cap. 2 > Economic Status].
- Cada nível acima/abaixo do normal: geração de carga **+8%/-8%** por prédio; em boom, **+16%** [SG > Cap. 2 > Cargo Generation Rate].
- Receita por carga: **+10%/-10%** por nível [SG > Cap. 2 > Cargo Revenues].
- Combinação (texto do guia): boom = 1,16 x 1,2 = **1,39** x receita normal; depressão = 0,84 x 0,8 = **0,67** x receita normal [SG > Cap. 2 > Cargo Revenues].
- Custo de trilho: **+10%/-10%** por nível [SG > Cap. 2 > Economic Status].
- Preço das ações: **+10%/-10%** por nível, "assumindo tudo o mais igual" [SG > Cap. 2 > Economic Status].
- Juros prime: "from 5 percent in boom times to 9 percent in depression" [SG > Cap. 2 > Economic Status].
- Bônus de estado "boom times": +20% (lista de bônus) [SG > Cap. 2 > Bonuses].

### 7.2 Cidades e regiões (economia local)

Ver seção 6.3 (crescimento de cidades, tamanho inicial, regiões).

## 8. Finanças

### 8.1 Início de empresa

- Dois controles: "Player Investment" (dinheiro próprio) e "Outside Investment" (investidores) [M > Cap. 2 > Starting a Company].
- Tutorial: ambos no máximo = $1.650.000 de caixa [M > Cap. 2 > Starting a Company].
- SG: "Usually, you can push both sliders all the way to the right to maximize your starting capital, but some scenarios will limit your options" [SG > Cap. 1 > Starting a Company].
- Quanto mais dinheiro próprio, mais investidores externos aceitam [SG > Cap. 1 > Starting a Company].
- Capital inicial de cenário (texto de dica): "$1,640,000 if you max it out" (Which Way to the Coast) [SG > Cap. 6 > Which Way to the Coast?].
- Empresa precisa existir antes de construir: "You must own a company BEFORE you can build track" [M > Cap. 4 > Lay Track > TIP].

### 8.2 Taxas, caixa e juros

- Taxa de juros recebida sobre caixa: "usually stays loosely around 50 percent of the Prime Rate"; é igual para todas as empresas [SG > Cap. 5 > Total Cash > Interest Rate].
- Juros anuais de caixa = caixa x taxa (usando média do ano quando a taxa muda) [SG > Cap. 5 > Annual Cash Flow].
- Juros de bônus: "the prime rate... Your credit rating also affects the actual rate" [SG > Cap. 2 > Other variables].
- Interest (despesa/receita de juros) = juros recebidos sobre caixa menos juros pagos sobre bônus [SG > Cap. 5 > Income > Expenses].
- Overhead: "about 1 percent to 3 percent of your revenue" [SG > Cap. 5 > Income > Expenses].
- Salários: soma do salário do chairman e do gerente (SG).

### 8.3 Bônus (bonds)

- Emissão: bônus em incrementos de **$500.000** [SG > Cap. 5 > Finances > Bonds].
- Só se pode emitir com rating **B ou melhor**. Juros do novo bônus = prime + **1%** por nível de rating abaixo de AAA [SG > Cap. 5 > Credit Rating].
- Prazo: "most of the campaigns end before 30 years, which is the payoff date on most bonds" [SG > Cap. 1 > Issuing Bonds].
- Falência: todos os bônus são reduzidos à metade [SG > Cap. 5 > Bonds].
- M: "You won't be able to issue bonds unless you have a solid credit rating of at least B or better" [M > Cap. 7 > Finances > Issue Bonds].

### 8.4 Rating de crédito

- Escala: F (pior) a AAA (melhor) [M > Cap. 7 > Finances; SG > Cap. 5 > Credit Rating].
- Fatores: razão dívida/ativos, cobertura de dívida, média dos dois mais bônus de gerente. Penalidade por falência que "wear off gradually over ten years"; empresas com menos de 5 anos têm penalidade [SG > Cap. 5 > Credit Rating Calculation].
- Razão dívida/ativos = Total Debt (bônus + caixa negativo) / total de ativos. Exemplo: 1.000.000 / 4.000.000 = 0,25, "roughly AA" [SG > Cap. 5 > Credit Rating Calculation].
- Tabela 5-5 (razão -> rating): 0 a 0,24 = AAA; 0,25 a 0,37 = AA; 0,38 a 0,49 = A; 0,5 a 0,57 = BBB; 0,58 a 0,66 = BB; 0,67 a 0,74 = B; 0,75 a 0,8 = CCC; 0,81 a 0,86 = CC; 0,87 a 0,95 = C; acima de 0,95 = F [SG > Cap. 5 > Table 5-5].
- Cobertura de dívida = (Earnings + Interest Paid) / Interest Paid. Exemplo: (100K + 300K) / 100K = 4,0. Média ponderada dos últimos anos, com peso maior no mais recente [SG > Cap. 5 > Credit Rating Calculation].
- Tabela 5-6 (metas de cobertura): **não está no HTML** (referida no texto, sem conteúdo extraível). Vago.
- "A bond issued in a company's first year can drop its credit rating several levels, from BBB to C" (exemplo) [SG > Cap. 5 > Credit Rating].
- Declaração de falência reduz rating; gerente com bônus de rating (ex.: Ames Oakes: "Credit Rating +1 level") [SG > Cap. 3 > The Managers].

### 8.5 Preço da ação

- Fórmula: **Preço da ação = Preço fundamental x Modificador de negociação (trade modifier)** [SG > Cap. 5 > Share Price Calculation].
- Pesos básicos do preço fundamental: Lucro (Earnings) **40%**; Valor contábil (Book Value) **30%**; Dividendos **20%**; Receita (Revenue) **10%**. Os pesos são ajustados pela tendência de cada critério [SG > Cap. 5 > Share Price Calculation].
- Valor por critério (por ação): Earnings = **10,0 x** lucro anual; Revenue = **1,0 x** receita anual; Book Value = **1,0 x** valor contábil; Dividends = **30 x** dividendo médio, com "big bonus for continuity" [SG > Cap. 5 > Share Price Calculation].
- Modificador de negociação: positivo se as ações estão sendo compradas e negativo se vendidas. "The minimum a share can go to is 30 percent of its fundamental price"; sem teto superior. O efeito de uma transação diminui com o tempo, com maior impacto nos primeiros seis meses [SG > Cap. 5 > Share Price Calculation].
- Manual: "Investors are very particular about Share Price—they want it to go up" (texto) [M > Cap. 7 > Finances].
- Razão de ações: "The Stock Price is determined mainly by the profitability" (texto) [SG > Cap. 5 > Share Price].
- Métricas de ações na tela Corporate (M, SG): Share Price (fim do ano); Book Value Per Share = (ativos - dívida) / ações; Revenue Per Share; Earnings Per Share; Dividend Per Share; "5 Year Weighted Return" = dividendos recebidos + variação de preço, média anual dos últimos 5 anos com peso maior ao recente. Fórmula de peso não documentada [M > Cap. 8 > Corporate].
- Efeito de evento "Stock Prices" (%): -99 a +100 [E3 > Appendix B].
- Gerentes com efeito em preço de ação (tabela de gerentes): Ginery Twitchell (-15%), Rudolph Diesel (-10%), Albert A. Robinson (-10%).

### 8.6 Dividendos

- Pagos trimestralmente; o jogador escolhe a taxa, mas "the board won't let you set the dividend higher than the company can afford" [SG > Cap. 5 > Finances > Change Dividend; M > Cap. 7 > Finances > Change Dividends].
- Dividendos "pagos ao fim de cada trimestre" (março, junho, setembro, dezembro) [M > Cap. 4 > Date].
- Teto do conselho: não documentado em valor.

### 8.7 Ações da empresa

- Emissão (Issue Stock): "Issuing stock drives your share price down" [SG > Cap. 1 > Issuing Stock].
- Recompra (Buy Back Stock): "Buying back stock from the market will drive your share price up" [SG > Cap. 1 > Buy Back Stock].
- Valor de emissão, número de ações por emissão e preço de referência: não documentados.
- Controle: "If you buy back enough stock, then you will have sole control of your railroad" [M > Cap. 7 > Finances > TIP].
- Percentual de ações: **50%** garante sucesso de fusão (merger); **10%** permite tentar assumir a presidência por eleição especial [SG > Cap. 5 > Balance > stock holdings].
- Fusão (Attempt Merger): uma vez por ano; votos dos acionistas da alvo; preço maior = mais votos; ações da própria empresa alvo podem votar a favor [M > Cap. 7 > Finances > Attempt Merger].

### 8.8 Falência (Declare Bankruptcy)

- "Half your debt will disappear" [M > Cap. 7 > Finances > Declare Bankruptcy].
- Consequências: sem empréstimos, perde todo o caixa da empresa, não emite bônus por um tempo, tem dificuldade para emitir ações [M > Cap. 7 > Finances].
- TSC: nova emissão de ações igual a **50%** das ações em circulação ("if you had 10,000 shares outstanding ... you'll have 15,000") [M > TSC > Gameplay Changes > Bankruptcy].
- Liquidação: "Normally, when a company goes broke, it is liquidated, but its track and stations stay on the map" (TSC: opção "Liquidate Bankrupt Companies" remove) [M > TSC > Game Options].
- Penalidade de rating: "Recent bankruptcies incur a heavy penalty ... wear off gradually over ten years" [SG > Cap. 5 > Credit Rating].

### 8.9 Patrimônio pessoal

- Patrimônio pessoal = caixa + valor das ações. Variável `PlayerNetWorth` = `PlayerCash` + `PlayerStockValue` [E3 > Appendix A].
- Poder de compra (purchasing power) = caixa + **metade** do valor das ações [M > Cap. 8 > Personal; SG > Cap. 1 > Personal Finances > TIP].
- Compra com margem (margin): pode tomar emprestado até metade do valor das ações ("borrow up to half of the value of your stock holdings") [M > Cap. 8 > Personal > Buying on Margin]. Poder de compra < 0 gera margin call (o corretor obriga a vender ações) [M; SG].
- Compra com margem e venda a descoberto (Short Selling) só existem no modelo financeiro Expert [M > Cap. 3 > Options > Financial model; M > Cap. 8].
- Modelo Advanced: venda a descoberto e margem desativadas; resto ativo [M > Cap. 3 > Options].
- Venda a descoberto: vende-se ações não possuídas "up to the extent of your purchasing power" [SG > Cap. 5 > Selling Stock].
- Compra de ação própria: ação da própria empresa só pode ser vendida se estiver em carteira [M > Cap. 8 > Personal].

### 8.10 Demonstrativos da empresa

- Income (receitas): Passengers (receita de passageiros) e Freight (demais cargas); Station (receita de upgrades e taxa de estação de outras empresas); Industry Investments (lucro das indústrias compradas); Miscellaneous [SG > Cap. 5 > Income].
- Income (despesas): Track, Engine, Car maintenance; Fuel; Engine maintenance; Interest; Overhead; Salaries; Miscellaneous [SG > Cap. 5 > Income].
- Balance: Cash, ativos físicos (prédios, trens, trilho, indústrias), dívida, patrimônio líquido ("Equity = assets - debt"). "Equity ... stays positive when your cash or annual income is negative" (texto) [SG > Cap. 5 > Balance].
- Territories: Political View, Access Rights, Goodwill (ver seção 8.11).
- Taxa de estação paga por outra empresa: $2.000 por vagão (SG > Income > Stations). Ver seção 4.7 sobre CONFLITO.
- Dívida "Debt" e caixa negativo: contam como dívida (E3, CompanyDebt) [E3 > Appendix A].

### 8.11 Territórios e direitos de acesso

- Quatro níveis de acesso (cores): Full (verde) = pode construir e rodar trens; Brown (marrom) = pode construir trilho e estações e comprar indústrias, mas não rodar trens; Yellow (amarelo) = pode rodar trens em trilho existente, sem construir nova infraestrutura; Red (vermelho) = nenhum direito [SG > Cap. 5 > Territories > Access Rights View].
- Direitos de trilho e de trem podem ser separados (Allow Separate Rights Purchases) [M > Cap. 10 > Territories].
- Goodwill: escala de 1 (horrível) a 100 (bom) no editor (Default Goodwill) [M > Cap. 10 > Territories].
- Goodwill é afetado por segurança e confiabilidade (cabooses, bons gerentes ajudam; quebras, acidentes e roubos prejudicam) e por confiabilidade do serviço (supply baixo sobe; surplus baixa) [SG > Cap. 5 > Goodwill View].
- Comprar direitos: via "Consult" (preço que o território aceita); ou via eventos [SG > Cap. 5 > Political View].
- Efeitos de evento: "Territory Allow ..." (all, track, train run, station build, building buy) e "Territory Confiscate" (instant) [E3 > Appendix B].
- Efeito de evento "Political Savvy" (%) [E3 > Appendix B].
- Contas por território em "CompanyTerritoryGoodwill" e "CompanyTerritoryHasTrainAccess" (E3 > Appendix A).

### 8.12 Jogador pessoal e gerentes

- Gerentes: salário mensal ("must be paid a monthly salary"), no máximo quatro bônus por gerente [SG > Cap. 3 > The Managers]. Tabela completa em Anexo G no fim do documento.
- Gerentes disponíveis: dois candidatos por vez, os melhores só aparecem quando a empresa vai bem [M > Cap. 7 > Overview > View/Hire other Managers].
- Efeito de evento "Manager Salaries" (%) e "(Manager Name) Availability" (Normal/IMMEDIATE/NEVER) [E3 > Appendix B].
- Chairman: "Resign as Chairman" mantém ações e riqueza; "Chairmen are appointed, not born" (texto) [M > Cap. 7 > Overview].
- Gastos de jogador com gerente são salário (e "Salaries" no Income).

### 8.13 Dicas de estratégia financeira (somente texto; sem valor de regra)

- Emissão de ações: "pause the game, then issue stock, and then buy it up" (para evitar que IA compre antes) [SG > Cap. 1 > Issuing Stock].
- Manipulação de dividendo e recompra (estratégia de enriquecimento pessoal) [SG > Cap. 9 > Maximizing Wealth].

---

## 9. IA, níveis de dificuldade e modelos de jogo

### 9.1 Modelos financeiro e industrial

| Modelo | Financeiro | Industrial | Fonte |
|---|---|---|---|
| Basic | Preço de ação pouco importante; pode comprar e vender ações da própria empresa, mas não pode tomar controle nem ser tomado | Estações aceitam qualquer carga, pagam um pouco menos pela não demandada; todas as indústrias produzem, mesmo sem insumos | [M > Cap. 3 > Options] |
| Advanced | Venda a descoberto e compra com margem desativadas; resto ativo | Estações aceitam qualquer carga, mas pela metade do preço se não demandada; indústrias só produzem com insumos | [M > Cap. 3 > Options] |
| Expert | Tudo ativo ("the gloves are off") | Não demandada paga "very little"; preço por nível de demanda (0 a 9); empresa pode comprar qualquer indústria | [M > Cap. 3 > Options] |
| Sandbox | Modelos financeiro e industrial desligados; dinheiro e concorrência não contam | | [M > Cap. 3 > Options] |

- Compra de indústria: SG diz só no Expert; M diz que aparece no modelo Advanced (ver seção 13).

### 9.2 Níveis de dificuldade (skill levels)

| Nível | Modelos | Modificador de receita do jogador humano | Modificador do computador |
|---|---|---|---|
| Easy | Financeiro Basic, industrial Basic | +20% | 10% |
| Medium | Advanced, Advanced | sem modificador | sem modificador |
| Hard | Expert, Expert | -10% | +10% |
| Expert | Expert, Expert | -20% | +20% |

[M > Cap. 3 > Skill Levels]. Texto ("Custom") para ajustes fora dessas predefinições.

- Campanha: "Normal" (experiente), "Easy" (iniciante), "Hard" (só após uma campanha completa); a dificuldade não muda depois de começar [M > Cap. 3 > New Campaign].
- Revenue modifier (sliders de receita, humano e computador) pode ser ajustado livremente; reduzir para o humano torna o jogo mais difícil [M > Cap. 3 > Options].
- Difficulty Rating: sobe ao adicionar computadores; jogador não escolhe quais IAs entram; em alguns cenários não se pode remover todas [M > Cap. 3 > Computer Players].
- Número de computadores: "Adding more computer players by clicking the up arrow drives the Difficulty Rating higher" (texto) [M > Cap. 3 > Computer Players].

### 9.3 Comportamento da IA

- Tendências dos tycoons (Tabela 3-1): sete colunas: Starts Company, Margin Buying, Short Selling, Investing in Others, Issue Bonds, Expand Rail, Industry Investment. "100 is average and 200 is maximum"; abaixo de 100 "rarely acted upon". Tabela completa em Anexo H. [SG > Cap. 3 > Table 3-1].
- Exemplos de texto: Jay Gould "will speculate relentlessly in the stock market"; Cecil Rhodes foca em expansão, "even to the point of bankrupting his railroad" (M > Cap. 7 > Player Detail; é texto de sabor com efeito de regra mínimo).
- IA, geral: expande em áreas planas e evita montanhas; quando constrói em montanha, usa grades muito íngremes; ganha dinheiro sobretudo com passageiros e correio; raramente faz todas as melhorias de estação [SG > Cap. 9 > Playing with the AI].
- IA não compra direitos de território sem ser instruída por evento [E3 > Effects > Events notes].
- Modelo Expert para a IA permite venda a descoberto e margem: "they know exactly what they are doing, and can bankrupt you" (texto) [M > Cap. 8 > Personal > Short Selling].
- Empresas de IA "have no chairman" são consideradas computador (E3 > Triggers).

### 9.4 Modo multijogador (resumo)

- Modos: TCP/IP e IPX (mais de dois jogadores); Modem e Serial (um contra um) [M > Cap. 9 > Connecting Your Game].
- Só o host escolhe mapa ou jogo salvo; todos aprovam as opções (luz ao lado do nome); ping em ms [M > Cap. 9 > Setting Up].
- Chat: TAB abre, ENTER envia [M > Cap. 9 > Playing the Game].
- Alt+Tab desconecta com frequência; evitar (RM > (4) L).
- "Out of Synch": o host salva e todos reiniciam pelo jogo salvo (RM > (4) M).

---

## 10. Eventos e condições de vitória

### 10.1 Anatomia de um evento

- Quatro elementos: Trigger (gatilho), Message Text (texto), Frequency (frequência), Effect (efeito). Mais Comments (sem efeito no jogo) [E3 > Event Editor Overview].
- Evento sem texto ocorre em silêncio; texto não aparece se o evento só afeta outro jogador ou empresa não controlada [E3 > Message Text].
- Tipo de mensagem: Dialog (OK), Newspaper (HEADLINE + SUB HEADLINE), Choice (dois botões, cada um com seu próprio efeito) [E3 > Message Text; M > Cap. 10 > Events].
- Literais de texto (TSC): (PLAYER), (COMPANY), (TERRITORY), (TRIGGER) = valor numérico formatado "2,421", (TRIGGER_MONEY) = "$2,421", (TRIGGER_TF) = verdadeiro/falso (diferente de 0 = verdadeiro) [E2 > EVENT TEXT].
- Vitória ou derrota usa a placa própria, não os diálogos [E3 > Message Text].
- Frequência: anual ou mensal (fim ou início do período); a cada vez que trilho ou estação é colocado ou removido; quando empresa é criada; ao iniciar o briefing [E3 > Frequency].

### 10.2 Gatilhos (triggers)

- Gatilho é uma expressão tipo "if" [E3 > Triggers]. Exemplo: `GameYear = 1875`.
- Constantes: inteiros (sem vírgula de milhar, sem $) [E3 > Trigger Elements].
- Comparadores: `>=`, `<=`, `>`, `<`, `=`, `!=`, `AND`, `OR` [E3 > Trigger Elements].
- Operadores: `+`, `-`, `*`, `/` (divisão por zero avalia como 1) [E3 > Trigger Elements].
- Precedência: funções, parênteses, `* /`, `+ -`, comparadores, `AND/OR`. Exemplos: `3 * 2 + 2 = 8` é verdadeiro; `3 * (2 + 2) = 8` é falso [E3 > Evaluation order].
- Testes com múltiplos alvos: "Test Against Multiple Companies / Players / Territories"; empresas/jogadores podem ser filtrados como humano ou computador (chairman humano = humano; computador ou sem chairman = IA) [E3 > Triggers].
- One Time Only: desativa o evento depois da primeira avaliação verdadeira; com múltiplos alvos, para na primeira verdadeira [E3 > Triggers].
- Ordem: alvos na ordem das listas; eventos na ordem da lista [E3 > Triggers]. "Events are evaluated from the top of the list down" [SG > Cap. 9 > Event editor, TIP].
- Funções: "over 70" (texto) e tabela de 84 entradas (Apêndice A). A tabela completa está em Anexo P no fim. Destaques:
  - Tempo: `GameYear`, `GameMonth`, `GameYearMonth`, `GameStartYear`, `GameYearsElapsedSinceStart`, `GameEconomicState` (0 a 4).
  - Jogo: `GameNumPlayers`, `GameNumCompanies`, `GameLoadsHauledThisYear/Lifetime`, `GameTrackCells`, `GameSingleTrackCells`, `GameDoubleTrackCells`, `GameElectricTrackCells`, `GameVariable1..4`.
  - Empresa: `CompanyCash`, `CompanyDebt` (bônus + caixa negativo), `CompanyTotalAssets`, `CompanyBookValue` (ativos - dívida), `CompanyRevenueThisYear/Lifetime`, `CompanyNetIncomeThisYear/Lifetime`, `CompanyLoadsHauledThisYear/Lifetime`, `CompanyGoodwill`, `CompanyVariable1..4`, `CompanyIndustryProfitsThisYear`, `CompanyIndustryInvestments`.
  - Jogador: `PlayerCash`, `PlayerStockValue`, `PlayerNetWorth` (caixa + ações), `PlayerControlsAnyCompany`, `PlayerControlledCompanyId`, `PlayerVariable1..4`.
  - Território: `TerritoryTrackCells`, `TerritoryLoadsHauledThisYear/Lifetime`, `TerritoryConnectedToTerritory(ID)`, `TerritoryConnectedToCity(ID)`, `TerritoryVariable1..4`.
  - Empresa x território: `CompanyTerritoryHasTrainAccess`, `CompanyTerritoryHasTrackAccess`, `CompanyTerritoryHasStationAccess`, `CompanyTerritoryHasBuildingBuyAccess`, `CompanyTerritoryHasAnyAccess`, `CompanyTerritoryGoodwill`, `CompanyTerritoryConnectedToTerritory(ID)`, entre outras.
  - Campanha: `CampaignChoice1To3`, `CampaignDifficulty1To3`.
  - Estação: `StationVisited(ID)`, `NumberOfStationsVisited`.
  - Aleatório: `Random1To10000`.
  - Classes: `IsHighestInClass`, `IsLowestInClass` (ex.: `CompanyCash(IsHighestInClass)`).
  - Novos no TSC: `CompanyIndustryProfitsLifetime`, `CompanyTerritoryLoadsHauledFromThisYear`, `CompanyTerritoryLoadsHauledFromLifetime`, `CompanyTerritoryLoadsHauledFromLifetimeToTerritory`, `CompanyTerritoryTypeLoadsHauledToThisYear/ToLifetime/FromThisYear/FromLifetime`, `CompanyTypeLoadsHauledThisYear/Lifetime`, `CompanyAverageSpeedThisYear/Lifetime`, `GameTrackCellsVisited`, `TerritoryCompletelyFlooded`, `CompanyTrackCellsRemaining`, `CompanyHasAnyAccessTerritoryParameter` [E3 > RT2 TSC; E2].
- Exemplo do guia: `GameYear >= 1872 AND (CompanyGoodwill>10 OR CompanyCash(IsHighestInClass) OR CompanyDebt * 3 < CompanyTotalAssets)` [E3 > Triggers].
- Tipos de carga numéricos (para funções "Type"): PASSENGERS 0, MAIL 1, LUMBER 2, LOGS 3, MILK 4, CATTLE 5, STEEL 6, GOODS 7, AUTOMOBILE 8, CEMENT 9, FOOD 10, COTTON 11, WOOL 12, PAPER 13, OIL 14, CHEMICALS 15, FERTILIZER 16, HAZARDOUS 17, COAL 18, GRAIN 19, ALUMINUM 20, COFFEE 21, TIRES 22, SUGAR 23, URANIUM 24, PULPWOOD 25, GRAVEL 26, IRON 27, PRODUCE 28, BAUXITE 29, DIESEL 30, RUBBER 31, TROOPS 32, AMMO 33, WEAPONS 34, ALCOHOL 35, METRA_RES_TO_COM 36, METRA_RES_TO_IND 37, METRA_ANY_TO_RES 38, METRA_RES_TO_AIR 39, METRA_COM_TO_AIR 40, METRA_AIR_TO_COM 41 [E3 > RT2 TSC; E2].

### 10.3 Efeitos (effects)

- "approximately 250 effects" (texto) e tabela de 167 entradas no Apêndice B. Anexo Q no fim [E3 > Appendix B].
- Categorias: Performance (inclui Game Over), Cargo & Items (produção e valores de carga) e Variables (variáveis) [E3 > Effects].
- Efeitos com valor em percentual: "Variable % -99 a +100" (vários: Acceleration, Bridge Building, Building Buy Cost, Company Overhead, Engine Purchase e Maintenance e Fuel por tipo de motor, Political Savvy, Station Building, Station Turnaround, Stock Prices, Track Building, Track Maintenance, Traction e Train Speed por tipo, Train Safety, produção e receita por carga) [E3 > Appendix B].
- Efeitos com nível: Credit Rating (-10 a +10 níveis), Prime Rate (-5 a +5), Economic Status (-4 a +4) [E3 > Appendix B].
- Efeitos "instant": não são desligados por "Temporary"; só podem ser revertidos por outro evento. Exemplo: `GameYear>=1880`, efeito `Territory Confiscate All`; depois `GameYear>=1885`, `Territory Confiscate None` [E3 > Instant].
- Duração: Permanent (até outro efeito mudar) ou Temporary (anos e meses; "You may respond in months and the program will automatically divide them into years and months") [E3 > Duration].
- Variáveis: Global (uma para cada numeração 1 a 4), Company, Player, Territory; cada uma com "Set" e "Add To"; faixa -99 a +100 [E3 > Effects > Variables].
- Efeitos de disponibilidade: "(Locomotive Name) Availability" e "(Manager Name) Availability" com Normal, IMMEDIATE ou NEVER [E3 > Appendix B].
- Efeitos de Game Over: Win 1, Win 2, Win 3, Loss (on/off) [E3 > Appendix B]. Rótulos conflitam (ver seção 13).
- Efeitos de território: Territory Allow all / Track Build / Train Run / Station Build / Building Buy (All or Nothing); Territory Confiscate All / Track / Trains / Stations / Buildings (None or All) [E3 > Appendix B].

### 10.4 Condições de vitória

- Tutorial (RT2 base): $10 milhões de patrimônio pessoal até 1900; $25 milhões ou $50 milhões dá vitória maior [M > Cap. 2 > Exploring the Main Interface]. RM (versão 1.55): "takes only $1 million in personal net worth to win this" (CONFLITO; ver seção 13).
- Campanha RT2 (The Iron Seed e 17 cenários seguintes): Bronze, Silver e Gold com metas por cenário. Tabela completa em Anexo N (no fim).
- Cenários avulsos: 12 metas (Bronze, Silver, Gold). Anexo O (no fim).
- Campanha TSC (The Second Century): 18 cenários em 3 capítulos de 6, jogados em ordem (podem ser perdidos e revisitados; pode-se "resign a scenario") [M > TSC > Campaign - The Second Century]. Metas dos cenários TSC não estão extraídas nas fontes lidas (ver seção 13 - lacunas).
- Cenários TSC com objetivos operacionais além de dinheiro (guerra, desastre, metrô): "Financial wizardry alone will not ensure victory in RT2-TSC" [M > TSC > What's New].
- Medalhas cumulativas: a medalha superior exige as metas da inferior mais as suas (ex.: cenário 1, Silver = "In addition to the above, connect to Harper's Ferry and haul at least 8 loads") [SG > Cap. 6 > Award Objectives]. Medalhas do motor (Win 1/2/3) em E3 ver seção 13.
- Bônus de cenário: escolha no início de cada cenário de campanha, entre opções listadas em Anexo N [M > Cap. 3 > New Campaign].
- Restrições do editor (Restrictions): lista de ações proibidas no mapa [M > Cap. 10 > Restrictions].

---

## 11. Editor de mapas e cenários

- Entrada: menu Editor > New Map (width e height) ou Load Map; From Scratch ou From Image [M > Cap. 10 > Starting a New Map].
- Mapas do jogo carregados no editor e salvos "won't play correctly in the game" [M > Cap. 10 > Loading a Map].
- Tamanho: 125.000 quadrados no máximo sem mais de 16 MB de RAM [M > Cap. 10 > Importing an Image].
- Ferramentas da esquerda (9) e da direita (6): Raise/Lower Ground, Paint Terrain (Solid, Mix), Fill Terrain (All/Adjacent; Match/Above/Below), Add Trees, Territories, Lay Track, Bulldoze Objects, Build a New Station, Build new Building, Undo, Map Options, Buy Train, Stock Market, Control Panel, File Options [M > Cap. 10 > Editor Basics].
- Build New Building: Add City (Type, Name, Size, Architecture; Industry Weighting), Add Region (mesmo, mais densidade), Add Label, Lay River, Reserve Cells [M > Cap. 10 > Build New Building].
- Industry Weighting: 0 = nunca; 200 = "twice the normal chance" [M > Cap. 10 > Build New Building].
- Territórios: nome; Default Goodwill (1 a 100); Border Visible; Allow Separate Rights Purchases; Buyable [M > Cap. 10 > Territories].
- Map Options:
  - General: Description; Start Year, ou Minimum e Maximum Start Year.
  - Events: lista de eventos (seção 10).
  - Players: slots; Mandatory ou Optional; Human, Computer ou Either; caixa inicial.
  - Player Pool: North American, European ou World.
  - Managers: gerentes permitidos.
  - Economy: Starting Economic Level; Annual Growth Rate ("Follow the recommended settings").
  - Engines: motores que aparecem no mapa.
  - Industries: restringe os tipos de indústria usados na geração aleatória de cidades e regiões.
  - Restrictions: ações proibidas.
  - Ports: oferta e demanda globais; "the demand must be delivered to generate the supply".
  - Robbers: Overall Robbery Level (~200 = era Jesse James; ~30 = mais tranquilo).
  [M > Cap. 10 > Map Options]
- Rios: só um tipo; quem não estiver em rio não pode pontear água [M > Cap. 10 > Lay River].
- Territórios invisíveis servem para eventos (não aparecem na página de territórios) [SG > Cap. 9 > Territories variables].
- Efeitos de editor de TSC: restrições especiais para metrô e tempo de guerra; frequência de atualização do status do annual report [M > TSC > Editor Changes].

---

## 12. Interface: telas e atalhos

### 12.1 Menu principal

- Single Player (New Campaign, Load Campaign, New Scenario, Load Scenario), Multiplayer, Editor, High Scores, Credits, Quit [M > Cap. 3 > Main Menu Options].
- TSC: Single Player > New Campaign > Second Century > dificuldade [M > TSC > Campaign - The Second Century].
- New Scenario: Map (com data inicial ajustável, se o mapa permitir), Options (Sandbox, Financial model, Industrial model, Revenue modifier), Skill Levels, Computer Players [M > Cap. 3 > New Scenario].

### 12.2 Tela principal

- Quatro partes: Main window (A), Radar (B), Center List Box (C), Activity Box (D) [M > Cap. 4 > The Four Parts].
- Oito botões à esquerda; os quatro primeiros são modos de ação (Lay Track, Build Station, Bulldoze, Map Overview), que se desligam ao clicar de novo; os dois seguintes compram trem e vão à Bolsa; os dois últimos abrem Control Panel e File Options [M > Cap. 4 > The Primary Action Buttons].
- Quatro botões ovais (girar e zoom) sobre o Activity Box; bússola no canto superior esquerdo gira o mapa [M > Cap. 4 > Navigating the Map].
- Center List Box: abas Stations, Trains, Players, Companies; botão de lista detalhada (List Screen); caixa de caixa (cash) e data [M > Cap. 4 > The Center List Box].
- Caixa de caixa: clicar alterna para caixa pessoal [M > Cap. 4 > Cash Display].
- Map Overview: Resource Map, Station Statuses, Track Grade Map, Traffic Density, Track by Company [M > Cap. 4 > Map Overview].
- Em Lay Track: custo branco por trecho, grade verde/amarela/vermelha [M > Cap. 2 > Laying Your First Track].

### 12.3 Control Panel (Painel de controle)

- General: Game Speed; Sound Volume (0 a 100%); Music Volume (0 a 100%); Color Depth (16 ou 8 bits, "about 10 percent slower" com 16); Memory Usage (alto se >= 32 MB de RAM); Full Screen; Grid Lines; Color Mouse Cursors [M > Cap. 4 > Control Panel > General].
- Graphics: Overall Detail (Very Low a Very High, cinco ajustes automáticos); sliders; Border; seis níveis de zoom [M > Cap. 4 > Control Panel > Graphics].
- Game Play: Arrival Bell (frequência e volume); News Frequency; Train Logos (All, Foreign ou None); Keep game going in dialogs (padrão: tempo para em dialogs); Dialogs disappear if left alone; Game Tips; Tutorial [M > Cap. 4 > Control Panel > Game Play].
- TSC: Autosave (intervalo escolhido, nome "Autosave"); Liquidate Bankrupt Companies [M > TSC > Game Options].

### 12.4 Telas de detalhe

- Lista de estações/trens/jogadores/empresas: duplo clique abre a tela de detalhe; botões de seta passam de item a item [M > Cap. 4 > Center List Box; M > Cap. 2 > TIP].
- Train Detail Screen: Instrument Panel (óleo, areia, água, throttle), Route/Consist List Box (paradas, semáforo, Delete), Train/Station Info, Map Router / Car List [M > Cap. 6].
- Station Detail Screen: 3D da estação, botão Buy (melhorias), Supply/Demand, To/From [M > Cap. 5 > Station Detail Screen].
- Station List Screen: cinco estações por vez, com supply/demand [SG > Cap. 5 > Station List Screen].
- Player Detail: caixa, ações, poder de compra; clique na ação abre a Bolsa [SG > Cap. 1 > Personal Finances].
- Company Detail: Overview, Income, Balance, Territories (Political View, Access Rights, Goodwill), Finances [M > Cap. 7; SG > Cap. 5].
- Stock Market: abas Personal (Buy, Sell, View) e Corporate [M > Cap. 8 > The Stock Market Screen].
- Purchase Train: lista de motores, atributos à direita (custo, manutenção, combustível, aceleração, confiabilidade, matriz de velocidade) [M > Cap. 6 > Train Purchase Window].

### 12.5 Atalhos de teclado e mouse

| Ação | Tecla / mouse | Fonte |
|---|---|---|
| Pausar / alternar pausa e velocidade 3 | PAUSE (pressionar duas vezes alterna pausa e velocidade 3) | [M > Cap. 2 > Exploring the Main Interface] |
| Aumentar / diminuir velocidade | + / - | [M > Cap. 2; M > Cap. 4 > Game Speed] |
| Cancelar trilho proposto (durante o arrasto) | ESC | [M > Cap. 2 > Laying Your First Track] |
| Cancelar edição de texto / pular campo | ESC (cancela); ENTER (aceita) | [M > Cap. 9 > Change] |
| Confirmar nome da empresa | ENTER | [M > Cap. 2 > Starting a Company] |
| Chat em multiplayer | TAB, digitar, ENTER | [M > Cap. 9 > Playing the Game] |
| Cheat "nowreck" (sem acidentes) | TAB e digitar `nowreck`; ou parâmetro `/nowreck` | [HS > 1.53 D] |
| Adicionar waypoint (TSC) | Ctrl + arrastar no trilho | [M > TSC > Route Waypoints] |
| Estação: mostrar área, supply e demand | Passar o cursor (área acende); clicar e segurar com botão direito na estação | [M > Cap. 4; SG > Cap. 9 > Finding Station Information] |
| Estações: mostrar Station Statuses (World Overview) | tecla 7 (fora do teclado numérico) | [SG > Cap. 5 > World Overview] |
| Centralizar a visão | Clicar no radar | [M > Cap. 4 > Checking Your Radar] |
| Rolagem | Mover o cursor para a borda da tela; no radar, arrastar | [M > Cap. 2 > Map Overview; HS > 1.56] |
| Zoom | Roda do mouse (1.56) | [HS > 1.56 A] |
| Rolagem por arrasto com botão do meio | Pressionar e arrastar | [HS > 1.56 A] |
| Ver detalhes do trem / do prédio | Clique direito | [M > Cap. 4 > Viewing Buildings; M > Cap. 4 > Center List Box] |
| Abrir detalhe de item da lista | Duplo clique | [M > Cap. 2] |
| Mudar classe do trem (Express, Normal, Slow, Halted) | Clicar no ícone à esquerda do trem na lista | [M > Cap. 2 > Tips] |
| Mudar semáforo de carga da parada | Clicar no semáforo na lista de rota | [M > Cap. 6] |
| Editor: mostrar grade | CTRL+G | [SG > Cap. 9 > Adding Rivers] |
| Editor: nivelar área (Smooth Out) | Segurar SHIFT e mover o cursor | [SG > Cap. 9 > Tools] |
| Evitar em multiplayer | Alt+Tab (causa desconexão) | [RM > (4) L] |

---

## 13. Conflitos e lacunas

### 13.1 Conflitos entre fontes

1. **Velocidade sem areia**: SG (tabela de melhorias) diz "Trains without sand travel 50% slower"; SG (texto) diz "Without sand, trains will run twice as slow"; M e SG (texto de grade) dizem "a 2% grade acts like a 4% grade".
2. **Limite de grade "íngreme"**: M diz "a grade greater than 4.0 is considered steep"; SG define verde 0 a 1,9%, amarelo 2 a 3,9%, vermelho 4% e acima.
3. **Idade e quebra**: M diz que um motor de 20 anos tem "about a 60 percent higher risk than a 5-year-old"; SG diz "A 10-year-old train is 33 percent more likely ... A 20-year-old train is 66 percent more likely" (relativo a novo).
4. **Substituir motor**: M sugere 15 a 25 anos; SG sugere 10 a 20 anos.
5. **Carga sem demanda (preço)**: SG diz 80/50/30% (Easy/Medium/Hard); AG Tabela A-3 diz "None" = 80/50/20% (Basic/Advanced/Expert). Nomes de modelos diferentes.
6. **Taxa de estação alheia**: M (Cap. 6) diz $2.000 por carga carregada ou entregue em estação de concorrente; SG (Cap. 2) diz $1.000 por carga na origem e $1.000 no destino; SG (Cap. 5, Income) diz $2.000 por vagão cobrado de outra empresa.
7. **Spanning bridge**: RM diz $120.000 por seção; SG (TSC) diz $150k por seção; RM diz "must be straight" e "only single track"; SG concorda com via simples.
8. **Rótulos de vitória**: E3 (Game Over - Win 1, 2, 3) diz "bronze, silver, gold"; AG diz Win 1 = "last place finish", Win 2 = "second place", Win 3 = "first place".
9. **Compra de indústria**: SG diz que só existe no modelo Expert; M (Cap. 4) diz que aparece com o modelo industrial Advanced.
10. **Vitória do tutorial**: M: $10 milhões até 1900 (e $25/50 milhões para vitória maior); RM (1.55): "only $1 million" (RM diz que foi reduzida).
11. **Lançamento do TGVx**: M (tabela de motores) = 1957; HS (1.51) muda a disponibilidade de 1973 para 1955.
12. **Motores: custos e manutenção**: tabela 4-1 do M e Tabela A-1 do AG divergem em vários modelos. Exemplos: Trevithick, combustível 330 (M) x 7.543 (AG); John Bull, custo 13.000 (M) x 23.000 (AG); Prussia, custo 14.000 (M) x 35.000 (AG); Dash-9, manutenção 32.000 (M) x 18.000 (AG); DB 18-201, combustível 140.000 (M) x 52.000 (AG); GP9, manutenção 15.000 (M) x 11.000 (AG); Hudson, manutenção 21.000 (M) x 25.000 (AG). Uma fonte pode estar errada. Recomendação: usar AG Tabela A-1 como fonte primária (mais completa) e marcar as divergências para revisão.
13. **Dividendos**: M diz pagos "at the end of each quarter" (março, junho, setembro, dezembro); SG diz "in the next month after the quarter is over" (mesmo efeito prático, com defasagem de um mês em relação ao fim do trimestre).
14. **Tamanho da estação média e grande**: SG diz média = "2,9 vezes a área" da pequena e grande = "10,2 vezes" a pequena, mas diâmetros de 9 e 17 dão razões de 3,24 e 11,56. Os multiplicadores impressos não batem com a geometria. Usar geometria e registrar.
15. **Cidades x casas**: M diz town = 4 casas e city = 8 casas; SG (Tabela 11) descreve tamanho de população por número de prédios (Village 1 a 2; Town 1 a 4; City 4 a 8; Metropolis 6 a 11; Megalopolis 8 ou mais). Conceitos parecidos, mas diferentes.
16. **Cutoffs de carga**: a tabela de demanda usa "0" e "9999" nos campos de cutoff; o significado de 0 (nunca?) e de 9999 (sem fim?) não é explicado.
17. **Sheep Farm (texto)**: AG diz "Cotton plants demand wool"; trata-se de erro de texto; a tabela indica que a fazenda de ovelhas produz lã e a fábrica têxtil demanda algodão e lã.
18. **Grafias da tabela de motores (M, 4-1)**: "284,00", "265,00" (faltam dígitos) e "1,000 K", "2,500K" (unidade em K) são grafias do original. Corrigir com a fonte AG se possível.

### 13.2 Lacunas (não documentado nas fontes lidas)

- Túneis: nenhuma regra (custo, tipo, limite).
- Sinalização de via (semáforos de linha, sinais de bloqueio): não há regras; o "traffic signal" documentado é o semáforo de carga da parada.
- Fórmula de velocidade, aceleração e tração por motor (só matriz de valores).
- Fórmula exata de preço de carga em função da distância e do tempo (só tabela de fatores).
- Fórmula de retorno/lucratividade das indústrias (só faixas de rating).
- Tabela 5-6 do SG (metas de cobertura de dívida): mencionada no texto, ausente do HTML.
- Valor de emissão de ações (preço e número de ações por emissão).
- Teto de dividendo permitido pelo conselho (valor).
- Capacidade por vagão e unidade de carga além de "load" (um vagão carregado).
- Intervalo exato de frequência de quebra por idade além dos multiplicadores citados.
- Lista de bônus dos cenários avulsos (12) não aparece nos blocos extraídos.
- Metas dos cenários TSC: não aparecem nos blocos "Award Objectives" lidos; o capítulo "The Second Century" do SG traz dicas, não metas.
- Efeitos e funções do editor com descrição vazia no texto original (várias entradas de receita e produção por carga).
- Tabela de gerentes: a unidade do salário (mensal ou anual) é ambígua (o texto diz mensal; os números são de 3.000 a 94.000).

---

## Anexos (tabelas extraídas)

Tabelas extraídas automaticamente dos HTML e dos textos fonte. Conferir contra a fonte antes de usar em testes.

### Anexo A. Tabela A-1 do Apêndice: dados das locomotivas ("Locomotive Data")

Colunas: A/E/W (disponibilidade norte-americana/europeia/mundial), First/Last (anos em que está à venda), Cost, Maint. e Fuel (anuais), Accel. e Reliab. (categorias) e matriz 0@0%, 150@0%, 300@0%, 150@4%, 300@4% (toneladas @ grade %). Valores "base level", modificáveis por data, cenário, gerente e mapa [AG > Table A-1]. Fonte primária recomendada (ver seção 13.1, item 12).

| Name | A | E | W | First | Last | Cost | Maint. | Fuel | Accel. | Reliab. | 0@0% | 150@0% | 300@0% | 150@4% | 300@4% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Trevithick-1 |  | E |  | 1800 | 1835 | $10,000 | $6,000 | $7,543 | Extremely Poor | Extremely Poor | 10 | 5 | 2 | 1 | 1 |
| Stephenson Rocket | A | E | W | 1829 | 1851 | $16,000 | $6,000 | $7,992 | Extremely Poor | Below Average | 15 | 6 | 3 | 1 | 1 |
| 2-4-0 John Bull | A | E | W | 1831 | 1853 | $23,000 | $7,000 | $9,256 | Poor | Below Average | 25 | 11 | 5 | 1 | 1 |
| 0-4-0 Dewitt Clinton | A | E | W | 1833 | 1855 | $18,000 | $5,000 | $7,187 | Extremely Poor | Poor | 20 | 13 | 8 | 1 | 1 |
| 4-2-0 Prussian | A | E | W | 1837 | 1857 | $35,000 | $8,000 | $10,622 | Extremely Poor | Below Average | 30 | 16 | 8 | 1 | 1 |
| 4-4-0 American-C | A | E | W | 1848 | 1885 | $46,000 | $5,000 | $12,606 | Poor | Above Average | 42 | 26 | 15 | 1 | 1 |
| 4-2-2 Iron Duke | A | E | W | 1855 | 1895 | $78,000 | $9,000 | $30,736 | Poor | Below Average | 54 | 37 | 23 | 3 | 1 |
| 4-4-0 8-Wheeler | A | E | W | 1868 | 1903 | $59,000 | $5,000 | $17,808 | Below Average | Above Average | 48 | 33 | 20 | 3 | 1 |
| 2-4-0 Vulcan | A | E | W | 1872 | 1900 | $32,000 | $4,000 | $6,393 | Very Poor | Below Average | 30 | 20 | 12 | 2 | 1 |
| 2-8-0 Consolidation | A | E | W | 1877 | 1905 | $51,000 | $8,000 | $19,512 | Below Average | Good | 40 | 31 | 22 | 6 | 1 |
| 3-Truck Shay | A | E | W | 1882 | 1930 | $43,000 | $17,000 | $18,227 | Poor | Good | 15 | 14 | 13 | 10 | 7 |
| 4-8-0 Mastodon | A | E | W | 1890 | 1911 | $60,000 | $13,000 | $22,995 | Very Poor | Poor | 45 | 36 | 27 | 9 | 2 |
| 4-6-0 Ten-Wheeler | A | E | W | 1892 | 1911 | $66,000 | $11,000 | $21,727 | Below Average | Below Average | 50 | 38 | 26 | 6 | 1 |
| 2-6-0 Mogul | A | E | W | 1895 | 1915 | $83,000 | $12,000 | $25,131 | Average | Average | 50 | 38 | 27 | 7 | 1 |
| 1-3 BoBo | A | E | W | 1895 | 1935 | $85,000 | $6,000 | $33,116 | Poor | Poor | 55 | 44 | 33 | 13 | 3 |
| 4-4-2 Atlantic | A | E | W | 1902 | 1929 | $93,000 | $18,000 | $43,556 | Fast | Above Average | 80 | 49 | 28 | 3 | 1 |
| 2-6-0 Camelback | A | E | W | 1905 | 1932 | $75,000 | $9,000 | $15,000 | Poor | Very Good | 30 | 27 | 24 | 15 | 9 |
| 4-6-2 Pacific | A | E | W | 1908 | 1945 | $119,000 | $21,000 | $62,516 | Above Average | Average | 95 | 66 | 44 | 8 | 1 |
| 0-10-0 Class G10 |  | E | W | 1910 | 1939 | $98,000 | $38,000 | $50,521 | Above Average | Average | 50 | 47 | 34 | 9 | 2 |
| 2-6-2 Prairie | A | E | W | 1912 | 1953 | $85,000 | $11,000 | $34,157 | Below Average | Good | 60 | 49 | 35 | 9 | 2 |
| 4-4-0 D16sb | A | E | W | 1914 | 1934 | $65,000 | $9,000 | $21,024 | Poor | Very Good | 45 | 36 | 27 | 9 | 2 |
| 2-10-0 Class 13 H |  | E | W | 1917 | 1940 | $102,000 | $36,000 | $46,289 | Above Average | Good | 40 | 36 | 32 | 21 | 12 |
| USRA 0-6-0 | A | E |  | 1918 | 1942 | $90,000 | $13,000 | $29,993 | Poor | Good | 40 | 37 | 30 | 12 | 4 |
| 2-8-2 Mikado | A | E | W | 1919 | 1950 | $133,000 | $32,000 | $51,072 | Above Average | Good | 55 | 50 | 45 | 31 | 19 |
| Be 4/6 II |  | E | W | 1920 | 1950 | $61,000 | $11,000 | $14,701 | Fast | Very Good | 35 | 31 | 26 | 14 | 6 |
| 4-6-0 Class B12 | A | E | W | 1923 | 1939 | $146,000 | $14,000 | $32,407 | Fast | Above Average | 71 | 51 | 33 | 6 | 1 |
| Ee 3/3 | A | E | W | 1923 | 1948 | $47,000 | $7,000 | $11,880 | Very Poor | Good | 31 | 29 | 26 | 20 | 14 |
| Class 1045 | A | E | W | 1927 | 1950 | $95,000 | $6,000 | $11,636 | Fast | Above Average | 40 | 36 | 33 | 23 | 14 |
| USRA 0-8-0 | A | E | W | 1930 | 1948 | $98,000 | $18,000 | $32,236 | Below Average | Above Average | 45 | 40 | 35 | 20 | 10 |
| Class A4 Mallard |  | E | W | 1935 | 1955 | $200,000 | $19,000 | $55,136 | Above Average | Below Average | 126 | 92 | 53 | 6 | 1 |
| GG1 | A |  | W | 1935 | 1970 | $285,000 | $19,000 | $42,721 | Above Average | Outstanding | 100 | 87 | 74 | 43 | 20 |
| Class E18 |  | E | W | 1936 | 1966 | $97,000 | $16,000 | $27,493 | Fast | Good | 93 | 66 | 44 | 11 | 1 |
| 4-6-4 Hudson | A | E | W | 1937 | 1953 | $210,000 | $25,000 | $54,017 | Below Average | Average | 90 | 76 | 62 | 27 | 10 |
| 4-8-4 Daylight | A | E | W | 1937 | 1952 | $230,000 | $30,000 | $63,611 | Average | Below Average | 80 | 70 | 61 | 33 | 17 |
| 4-6-4 J3A Streamliner | A | E | W | 1938 | 1955 | $255,000 | $28,000 | $112,234 | Above Average | Poor | 103 | 89 | 67 | 23 | 6 |
| Ae 8/14 |  | E | W | 1939 | 1955 | $210,000 | $24,000 | $47,906 | Fast | Above Average | 68 | 64 | 60 | 46 | 34 |
| 4-8-8-4 Big Boy | A |  |  | 1941 | 1955 | $375,000 | $75,000 | $88,040 | Above Average | Poor | 68 | 67 | 62 | 48 | 35 |
| Class 1020 |  | E | W | 1941 | 1981 | $119,000 | $18,000 | $21,545 | Fast | Very Good | 56 | 51 | 45 | 31 | 18 |
| 4-4-4-4 T-1 | A |  | W | 1945 | 9999 | $284,000 | $28,000 | $78,213 | Fast | Good | 85 | 73 | 59 | 26 | 10 |
| F3A+B | A | E | W | 1945 | 1968 | $265,000 | $16,000 | $52,015 | Fast | Above Average | 85 | 79 | 65 | 30 | 12 |
| Alco PA-1 | A | E | W | 1946 | 1990 | $210,000 | $16,000 | $52,800 | Average | Average | 90 | 77 | 64 | 35 | 15 |
| F9 | A | E | W | 1949 | 1970 | $337,000 | $18,000 | $63,749 | Above Average | Above Average | 110 | 94 | 78 | 43 | 19 |
| GP9 | A | E | W | 1954 | 1980 | $165,000 | $11,000 | $48,028 | Average | Good | 71 | 64 | 56 | 36 | 20 |
| E 69 |  | E | W | 1955 | 1970 | $86,000 | $8,000 | $12,837 | Fast | Below Average | 31 | 29 | 28 | 24 | 19 |
| GP18 | A | E | W | 1958 | 1975 | $245,000 | $15,000 | $55,131 | Above Average | Very Good | 83 | 76 | 69 | 50 | 32 |
| V200 |  | E | W | 1959 | 1985 | $160,000 | $19,000 | $53,878 | Above Average | Below Average | 87 | 77 | 66 | 40 | 20 |
| Penn. E44 | A | E | W | 1960 | 1985 | $370,000 | $22,000 | $37,971 | Fast | Average | 70 | 63 | 56 | 38 | 22 |
| Class 55 Deltic |  | E | W | 1961 | 1981 | $480,000 | $15,000 | $52,155 | Below Average | Very Good | 100 | 87 | 73 | 41 | 19 |
| Shinkansen Bullet |  | E | W | 1966 | 2002 | $650,000 | $66,000 | $66,680 | Virtually Instant | Average | 130 | 93 | 61 | 16 | 2 |
| FP45 | A | E | W | 1968 | 9999 | $366,000 | $14,000 | $65,740 | Above Average | Above Average | 106 | 82 | 54 | 14 | 2 |
| SD45 | A | E | W | 1972 | 1994 | $360,000 | $21,000 | $36,009 | Average | Average | 65 | 59 | 53 | 37 | 22 |
| SDP40 | A | E | W | 1973 | 1989 | $292,000 | $18,000 | $61,966 | Above Average | Poor | 103 | 89 | 75 | 43 | 19 |
| E60CP | A |  |  | 1973 | 1998 | $260,000 | $19,000 | $38,043 | Very Fast | Average | 85 | 74 | 64 | 38 | 18 |
| Class E111 | A | E | W | 1974 | 9999 | $390,000 | $17,000 | $38,016 | Average | Good | 85 | 79 | 72 | 54 | 37 |
| E656 FS |  | E | W | 1975 | 1995 | $226,000 | $17,000 | $37,734 | Average | Above Average | 93 | 77 | 60 | 24 | 8 |
| Dash-9 | A | E | W | 1993 | 9999 | $478,000 | $18,000 | $68,410 | Above Average | Good | 70 | 65 | 60 | 46 | 31 |
| AMD-103 | A |  |  | 1993 | 9999 | $425,000 | $25,000 | $88,068 | Very Fast | Above Average | 105 | 86 | 68 | 32 | 11 |
| Thalys Bullet | A | E | W | 1994 | 9999 | $1,000,000 | $78,000 | $111,756 | Virtually Instant | Below Average | 186 | 133 | 88 | 23 | 3 |
| Class 232 | A | E | W | 1997 | 9999 | $492,000 | $35,000 | $72,625 | Very Fast | Very Good | 75 | 69 | 62 | 46 | 29 |
| Mag-Lev TBX-1 | A | E | W | 2008 | 9999 | $2,500,000 | $200,000 | $274,139 | Instant | Above Average | 280 | 192 | 120 | 26 | 3 |
| TGV-Prototype |  | E | W | 1955 | 1983 | $750,000 | $80,000 | $70,000 | Average | Poor | 155 | 90 | 50 | 8 | 2 |
| TGV-PSE |  | E | W | 1981 | 9999 | $875,000 | $77,000 | $93,000 | Fast | Good | 162 | 109 | 74 | 21 | 3 |
| E 412 'Brenner' | A | E | W | 1999 | 9999 | $1,250,000 | $170,000 | $85,000 | Average | Good | 140 | 128 | 108 | 50 | 30 |
| DB 18 201 | A | E | W | 1998 | 9999 | $400,000 | $75,000 | $52,000 | Above Average | Average | 120 | 90 | 53 | 20 | 4 |
| Eurostar |  | E | W | 1994 | 9999 | $1,500,000 | $180,000 | $100,000 | Average | Good | 186 | 160 | 100 | 20 | 3 |

### Anexo B. Tabela 4-1 do Manual: "The Iron Horses at a glance"

Colunas: Init. Cost, Maint. Cost, Fuel Cost, Speed (mph/kph) [M > Apêndice > Table 4-1]. Divergências com a Tabela A-1: ver seção 13.1, item 12.

| Locomotiva | Ano | Tipo | Custo inicial ($) | Manutenção anual ($) | Combustível anual ($) | Velocidade (mph/kph) |
|---|---|---|---|---|---|---|
| Trevithick 0-4-0 | 1804 | Steam | 10,000 | 6,000 | 330 | 5/8 |
| Rocket 0-2-2 | 1829 | Steam | 16,000 | 6,000 | 330 | 26/42 |
| John Bull 2-4-0 | 1831 | Steam | 13,000 | 5,000 | 340 | 18/29 |
| DeWitt Clinton 0-4-0 | 1833 | Steam | 12,000 | 5,000 | 340 | 15/24 |
| The Prussia 4-2-0 | 1837 | Steam | 14,000 | 5,000 | 350 | 27/43 |
| American 4-4-0 Class C | 1848 | Steam | 46,000 | 5,000 | 12,606 | 42/68 |
| Iron Duke 4-2-2 | 1855 | Steam | 78,000 | 9,000 | 30,736 | 54/86 |
| 8-Wheeler 4-4-0 | 1863 | Steam | 59,000 | 5,000 | 17,808 | 48/77 |
| Vulcan 2-4-0 | 1872 | Steam | 29,000 | 4,000 | 370 | 30/48 |
| Consolidation 2-8-0 | 1877 | Steam | 51,000 | 8,000 | 19,512 | 45/72 |
| 3-Truck Shay | 1882 | Steam | 43,000 | 17,000 | 18,227 | 12/19 |
| Mastodon 4-8-0 | 1890 | Steam | 60,000 | 13,000 | 22,995 | 45/72 |
| Ten Wheeler 4-6-0 | 1892 | Steam | 66,000 | 11,000 | 21,727 | 50/80 |
| 1-3 BoBo | 1895 | Electric | 85,000 | 6,000 | 33,116 | 60/97 |
| Mogul 2-6-0 | 1895 | Steam | 83,000 | 12,000 | 25,131 | 50/80 |
| Atlantic 4-4-2 | 1902 | Steam | 93,000 | 18,000 | 43,556 | 80/129 |
| Camelback 2-6-0 | 1905 | Steam | 75,000 | 9,000 | 15,000 | 30/48 |
| Pacific 4-6-2 | 1908 | Steam | 119,000 | 21,000 | 62,516 | 95/153 |
| Class G10 0-10-0 | 1910 | Steam | 98,000 | 38,000 | 50,521 | 50/80 |
| Prairie 2-6-2 | 1912 | Steam | 85,000 | 9,000 | 34,157 | 60/97 |
| American 4-4-0 | 1914 | Steam | 65,000 | 10,000 | 21,024 | 45/72 |
| Class XIII H 2-10-0 | 1917 | Steam | 102,000 | 36,000 | 46,289 | 40/65 |
| USRA 0-6-0 | 1918 | Steam | 90,000 | 13,000 | 29,993 | 40/64 |
| Mikado 2-8-2 | 1919 | Steam | 133,000 | 32,000 | 51,072 | 55/89 |
| Class Be 4/6 II | 1920 | Electric | 61,000 | 11,000 | 14,701 | 32/52 |
| Class B12 4-6-0 | 1923 | Steam | 146,000 | 14,000 | 32,407 | 71/114 |
| Ee 3/3 | 1923 | Electric | 47,000 | 7,000 | 11,880 | 31/50 |
| Class 1045 | 1927 | Electric | 95,000 | 6,000 | 11,636 | 40/60 |
| 0-8-0 | 1930 | Steam | 98,000 | 20,000 | 32,236 | 45/72 |
| Mallard 4-6-2 Class A4 | 1935 | Steam | 200,000 | 19,000 | 55,136 | 126/202 |
| GG1 | 1935 | Electric | 285,000 | 19,000 | 42,721 | 100/160 |
| Class E18 | 1936 | Electric | 97,000 | 16,000 | 27,493 | 93/150 |
| Daylight 4-8-4 | 1937 | Steam | 230,000 | 30,000 | 63,611 | 80/129 |
| Hudson 4-6-4 | 1937 | Steam | 210,000 | 21,000 | 54,017 | 90/145 |
| Class J3a 4-6-4 | 1938 | Steam | 255,000 | 23,000 | 28,000 | 103/166 |
| Class Ae 8/14 Landi-Lok | 1939 | Electric | 210,000 | 37,000 | 47,906 | 68/110 |
| Big Boy 4-8-8-4 | 1941 | Steam | 375,000 | 70,000 | 88,040 | 68/110 |
| Class 1020 Co-Co | 1941 | Electric | 119,000 | 23,000 | 21,545 | 56/90 |
| Penn. T1 4-4-4-4 | 1945 | Steam | 284,00 | 30,000 | 78,213 | 80/129 |
| F3, F3B | 1945 | Diesel | 265,00 | 16,000 | 52,150 | 85/137 |
| PA-1 | 1946 | Diesel | 210,000 | 16,000 | 52,800 | 100/160 |
| F7A-(F9 game +) | 1949 | Diesel | 337,000 | 15,000 | 63,000 | 120/164 |
| GP9 | 1954 | Diesel | 165,000 | 15,000 | 48,028 | 71/114 |
| E 69 02 | 1955 | Electric | 86,000 | 10,000 | 12,837 | 31/50 |
| GP18 | 1958 | Diesel | 245,000 | 15,000 | 53,878 | 83/134 |
| Class V200 (1953 orig.) | 1959 | Diesel | 160,000 | 19,000 | 53,878 | 87/140 |
| E44 | 1960 | Electric | 370,000 | 22,000 | 37,971 | 70/113 |
| Class 55 Deltic | 1961 | Diesel | 480,000 | 15,000 | 52,155 | 100/160 |
| Shinkansen Bullet Train | 1966 | Electric | 650,000 | 66,000 | 66,680 | 130/210 |
| FP45(p87, red book) | 1968 | Diesel | 366,000 | 22,000 | 65,740 | 103/166 |
| SD45 | 1972 | Diesel | 360,000 | 22,000 | 36,009 | 65/105 |
| E60CP | 1973 | Electric | 260,000 | 19,000 | 38,043 | 85/137 |
| SDP40 | 1973 | Diesel | 292,000 | 18,000 | 61,966 | 103/166 |
| Class E111 | 1974 | Electric | 390,000 | 17,000 | 38,016 | 85/137 |
| E656 Camino-FS | 1975 | Electric | 226,000 | 17,000 | 37,734 | 93/150 |
| Dash-9 | 1993 | Diesel | 478,000 | 32,000 | 68,410 | 70/113 |
| AMD-103 | 1993 | Diesel | 425,000 | 25,000 | 88,068 | 103/166 |
| Thalys | 1994 | Electric | 1,000 K | 40,000 | 111,756 | 186/300 |
| Class 232 | 1997 | Diesel | 492,000 | 35,000 | 72,625 | 75/120 |
| Mag-Lev TBX-1 | 2008 | Electric | 2,500K | 200,000 | 274,139 | 280/448 |
| Eurostar | 1994 | Electric | 1,600K | 65,000 | 140,000 | 168/270 |
| TGVx-Prototype | 1957 | Electric | 850,000 | 80,000 | 70,000 | 155/249 |
| TGV- Sud-Est | 1973 | Electric | 750,000 | 72,000 | 81,000 | 145/233 |
| DB 18-201 | 1998 | Steam | 400,000 | 75,000 | 140,000 | 120/193 |
| E-412 Brenner | 1999 | Electric | 1,250K | 170,000 | 70,000 | 137/220 |

### Anexo C. Tabela de cargas (Tabela 2-1 do Strategy Guide)

Fonte: [SG > Cap. 2 > Cargo Pricing > Table 2-1]. Significado das colunas: seção 5.1.

| Cargo | Base Price | Days to Deliver | Ship Distance | First Year | Cutoff for Towns | Cutoff for Cities | Demand per House | Storage Building |
|---|---|---|---|---|---|---|---|---|
| Passengers | 60,000 | 500 | 0.37 | 1800 | 9999 | 9999 | 0.35 |  |
| Mail | 70,000 | 300 | 0.47 | 1800 | 9999 | 9999 | 0.18 | Post Office |
| Lumber | 30,000 | 1,600 | 0.1 | 1800 | 9999 | 9999 | 0.1 |  |
| Logs | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 |  |
| Milk | 60,000 | 300 | 0.2 | 1800 | 1889 | 1889 | 0 | Refrigerated Storage |
| Cattle | 40,000 | 800 | 0.2 | 1800 | 0 | 0 | 0 |  |
| Steel | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 |  |
| Goods | 35,000 | 1,100 | 0.2 | 1800 | 9999 | 9999 | 0.2 |  |
| Autos | 35,000 | 1,100 | 0.2 | 1900 | 0 | 9999 | 0.2 |  |
| Cement | 35,000 | 1,100 | 0.1 | 1884 | 0 | 9999 | 0.1 |  |
| Food | 50,000 | 600 | 0.2 | 1800 | 9999 | 9999 | 0.1 |  |
| Cotton | 50,000 | 600 | 0.2 | 1800 | 0 | 0 | 0 | Warehouse |
| Wool | 50,000 | 600 | 0.2 | 1800 | 0 | 0 | 0 | Warehouse |
| Paper | 40,000 | 800 | 0.2 | 1800 | 0 | 9999 | 0.1 |  |
| Oil | 40,000 | 800 | 0.2 | 1860 | 0 | 1899 | 0.1 | Liquid Storage |
| Chemical | 40,000 | 800 | 0.2 | 1882 | 0 | 0 | 0 | Liquid Storage |
| Fertilizer | 30,000 | 1,600 | 0.1 | 1905 | 0 | 0 | 0 |  |
| Hazardous | 40,000 | 800 | 0.2 | 1950 | 0 | 0 | 0 |  |
| Coal | 30,000 | 1,600 | 0.1 | 1800 | 0 | 1899 | 0.15 |  |
| Grain | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 | Grain Silo |
| Aluminum | 30,000 | 1,600 | 0.1 | 1910 | 0 | 0 | 0 |  |
| Coffee | 40,000 | 800 | 0.2 | 1800 | 0 | 0 | 0 | Grain Silo |
| Tires | 30,000 | 1,600 | 0.1 | 1900 | 0 | 0 | 0 |  |
| Sugar | 40,000 | 800 | 0.2 | 1800 | 0 | 0 | 0 | Grain Silo |
| Uranium | 30,000 | 1,600 | 0.1 | 1950 | 0 | 0 | 0 |  |
| Pulpwood | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 |  |
| Gravel | 30,000 | 1,600 | 0.1 | 1884 | 0 | 0 | 0 |  |
| Iron | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 |  |
| Produce | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 | Refrigerated Storage |
| Bauxite | 30,000 | 1,600 | 0.1 | 1800 | 0 | 0 | 0 |  |
| Diesel | 30,000 | 1,600 | 0.1 | 1890 | 0 | 0 | 0 |  |
| Rubber | 30,000 | 1,600 | 0.1 | 1900 | 0 | 0 | 0 |  |
| Dining | 0 | 0 | 0.0 | 1846 | 0 | 0 | 0 |  |
| Caboose | 0 | 0 | 0.0 | 1839 | 0 | 0 | 0 |  |

### Anexo D. Tabela A-2 do Apêndice: calculadora de frete

[AG > Table A-2]. "By Towns" e "By Cities": demandada por towns e cities, com o último ano quando há corte. "Demand Modifiers" = melhoria de armazenamento relacionada.

| Cargo | Base Value | Delivery Time Days | Rot Factor* | By Towns | By Cities | Demand Modifiers** |
|---|---|---|---|---|---|---|
| ALUMINUM | $35,000 | 1300 | 3 |  |  |  |
| AUTOMOBILE | $49,000 | 800 | 6 |  | Yes |  |
| BAUXITE | $28,000 | 1600 | 2 |  |  |  |
| CATTLE | $37,000 | 800 | 5 |  |  |  |
| CEMENT | $38,000 | 800 | 3 |  | Yes |  |
| CHEMICAL | $37,000 | 800 | 5 |  |  | Liquid storage |
| COAL | $28,000 | 1600 | 2 |  | Until 1910 |  |
| COFFEE | $37,000 | 800 | 5 | Until 1878 | Until 1878 | Grain silo |
| COTTON | $36,000 | 1100 | 4 |  |  | Warehouse |
| DIESEL | $39,000 | 1100 | 4 |  |  |  |
| FERTILIZER | $35,000 | 1300 | 3 |  |  |  |
| FOOD | $58,000 | 600 | 6 | Yes | Yes |  |
| GOODS | $42,000 | 800 | 5 | Yes | Yes | Warehouse |
| GRAIN | $28,000 | 1600 | 2 |  |  | Grain silo |
| GRAVEL | $28,000 | 1600 | 2 |  |  |  |
| HAZARDOUS | $46,000 | 800 | 5 |  |  |  |
| IRON | $28,000 | 1600 | 2 |  |  |  |
| LOGS | $28,000 | 1600 | 2 |  |  |  |
| LUMBER | $35,000 | 1300 | 3 | Yes | Yes |  |
| MAIL | $70,000 | 300 | 10 | Yes | Yes | Post office |
| MILK | $55,000 | 300 | 5 | Until 1900 | Until 1900 | Refrigerated storage |
| OIL | $37,000 | 1100 | 3 |  | Until 1910 | Liquid storage |
| PAPER | $46,000 | 800 | 5 |  | Yes |  |
| PASSENGER | $60,000 | 500 | 8 | Yes | Yes |  |
| PRODUCE | $42,000 | 600 | 6 | Until 1878 | Until 1878 | Refrigerated storage |
| PULPWOOD | $28,000 | 1600 | 2 |  |  |  |
| RUBBER | $28,000 | 1600 | 2 |  |  |  |

### Anexo E. Demanda por nível: Tabela A-3 (AG) e Tabela 2-2 (SG)

Tabela A-3 em [AG > Table A-3]; Tabela 2-2 em [SG > Cap. 2 > Table 2-2] (percentual do preço cheio por nível de demanda, modelo Expert).

| Demand Level | Basic | Advanced | Expert |
|---|---|---|---|
| None | 80% | 50% | 20% |
| 0 | 100% | 100% | 50% |
| 1 | 100% | 100% | 60% |
| 2 | 100% | 100% | 70% |
| 3 | 100% | 100% | 80% |
| 4 | 100% | 100% | 90% |
| 5 | 100% | 100% | 100% |
| 6 | 100% | 100% | 110% |
| 7 | 100% | 100% | 120% |
| 8 | 100% | 100% | 130% |
| 9 | 100% | 100% | 140% |

| Percentage of Full Price | Demand Level |
|---|---|
| 50% | 0 |
| 60% | 1 |
| 70% | 2 |
| 80% | 3 |
| 90% | 4 |
| 100% | 5 |
| 110% | 6 |
| 120% | 7 |
| 130% | 8 |
| 140% | 9 |

### Anexo F. Tabela de indústrias ("The Industries")

[M > Apêndice > The Industries]. Colunas: Purchase $, Demands, Produces, Qty. Annually, Bonuses. Observações sobre a leitura da tabela: seção 6.

| Indústria | Compra ($) | Demanda | Produz | Qtd. anual (texto do manual) | Bônus | Disponibilidade / observação (texto do manual) |
|---|---|---|---|---|---|---|
| Aluminum Mill | 500.000 | Bauxite | Aluminum | N/A | — | — |
| Auto Plant | 600.000 | Steel + Tires | Autos | N/A | — | Autos demandadas por qualquer cidade; "the larger the city, the greater the demand" |
| Bakery | 300.000 | Grain, Sugar | Food | N/A | — | — |
| Bauxite Mine | 150.000 | — | Bauxite | 2 | — | — |
| Cannery | 250.000 | Produce + Steel (*) | Coffee or Food (canned) | N/A | — | (*) layout ambíguo no HTML; ver nota |
| Cattle Yard | 150.000 | — | Cattle | 3 | Grain (Prod. + 50%) | Grain aumenta produção em 50% |
| Cement Plant | 400.000 | Gravel | Cement | N/A | — | — |
| Chemical Plant | 300.000 | — | Chemicals | 2 | — | "very lucrative" (texto) |
| Coal Mine | 300.000 | — | Coal | 2 | — | — |
| Coffee Farm | 150.000 | — | Coffee | 2 | Fertilizer (Prod. + 50%) | "after 1876 it has to go through a cannery" |
| Cotton Farm | 200.000 | — | Cotton | 2 | Fertilizer (Prod. + 50%) | "one of the earliest industries" |
| Dairy Farm | 200.000 | — | Milk | 2 | Grain (Prod. + 50%) | Após ~1880 leite só aceito via Dairy Processor (texto) |
| Dairy Processor | 300.000 | Milk | Food | N/A | — | "doesn't exist until around 1880" |
| Electric Plant | 400.000 | Coal, Diesel | Waste | N/A | — | "doesn't appear in the game until 1890"; produz energia elétrica (não transportável) |
| Fertilizer Plant | 300.000 | Chemicals | Fertilizer | N/A | — | "become available in 1905" |
| Grain Silo | 300.000 | — | Grain | 2 | — | Abastece padarias e fazendas de animais (texto) |
| Gravel Pit | 200.000 | — | Gravel | 2 | — | "becomes available in 1884" |
| Housing | N/A | — | Passengers/Mail | 0.5/0.2 | — | Casas "produce" passageiros e correio anualmente |
| Iron Ore Mine | 200.000 | — | Iron | 2 | — | — |
| Landfill | 300.000 | Waste | — | N/A | — | "one of the last industries available" |
| Logging Camp | 150.000 | — | Logs/Pulpwood | 2/1 | — | — |
| Lumber Mill | 200.000 | Logs | Lumber | N/A | — | — |
| Meatpacking Plant | 400.000 | Cattle | Food | N/A | — | — |
| Nuclear Plant | 1.000.000 | Uranium | Waste | N/A | — | "not available until 1950"; produz energia elétrica |
| Oil Refinery | 500.000 | Oil | Diesel | N/A | — | — |
| Oil Well | 200.000 | — | Oil | 3 | — | "directly to cities after 1890" (texto) |
| Paper Mill | 300.000 | Pulpwood | Paper | N/A | — | "available in 1800" |
| Port | 300.000 | Varies | Varies | N/A | — | Demanda/produção definidas no editor de mapas; até 4 conjuntos por mapa |
| Produce Orchard | 200.000 | — | Produce | 2 | Fertilizer (Prod. + 50%) | "available in 1800"; até 1876 envia a cidades, depois a canneries |
| Rubber Farm | 150.000 | — | Rubber | 2 | Fertilizer (Prod. + 50%) | "won't find any until 1900" |
| Sheep Farm | 200.000 | — | Wool | 2 | Grain (Prod. + 50%) | "available in 1800"; "Shipping grain ... increase production 50 percent" |
| Steel Mill | 800.000 | Coal + Iron | Steel | N/A | — | — |
| Sugar Farm | 200.000 | — | Sugar | 3 | Fertilizer (Prod. + 50%) | "available in 1800" |
| Textile Mill | 400.000 | Cotton, Wool | Goods | N/A | — | "available in 1800" |
| Tire Factory | 300.000 | Rubber | Tires | N/A | — | "Available in 1900" |
| Tool and Die Factory | 200.000 | Iron, Steel, Aluminum | Goods | N/A | — | "available in 1800" |
| Uranium Mine | 600.000 | — | Uranium | 2 | — | "unavailable until 1950" |

Nota: a linha do Cannery no HTML traz "Produce + Steel" em linha separada da coluna de produção; a leitura acima (demanda = Produce + Steel; produz = Coffee or Food (canned)) é a mais coerente com a descrição, mas é inferência de layout.
Nota: no texto da indústria Sheep Farm o manual diz "Cotton plants demand wool" (erro de texto do manual; a tabela indica Sheep Farm produz Wool e Textile Mill demanda Cotton, Wool).
Nota: "Qtd. anual" é a coluna "Qty. Annually" do manual, definida como "the number of cars filled annually"; o manual não diz a unidade exata nem o significado de "N/A" além de "dependente da demanda".

### Anexo G. Gerentes: salário e bônus (Tabela 3-2 do SG)

[SG > Cap. 3 > Table 3-2]. "Salary" conforme impresso; o texto diz pagamento mensal (ver seção 13.2).

| Name | Salary | Bonus 1 | Bonus 2 | Bonus 3 | Bonus 4 |
|---|---|---|---|---|---|
| Albert A. Robinson | 19,000 | Track building-15% | Stock Price10% lower |  |  |
| Allen Mac Nab | 10,000 | Company Overhead -10% | Goodwill +10% |  |  |
| Ames Oakes | 8,000 | Engine Purchase+12% | Credit Rating +1 level | Goodwill +10% | Station Building -10% |
| Andre Chapelon | 36,000 | All Engine Speed +10% |  |  |  |
| Andrew Carnegie | 52,000 | Bridge Building -30% | Station Building-15% | Steel Revenue +10% |  |
| Bat Masterson | 38,000 | Train Safety +50% | Passenger Revenue +5% | Station Revenue+5% |  |
| Ben Holladay | 26,000 | Company Overhead -15% | Mail Revenue +10% |  |  |
| Charles F. Mayer | 89,000 | Bridge Building -20% | Track Building -40% |  |  |
| Charles M. Hayes | 77,000 | Company Overhead -5% | Track Building -40% | Train Safety -5% |  |
| Daniel Gooch | 44,000 | Company Overhead -5% | All Engine Purchase -20% | Track Building -5% | Iron Revenue +20% |
| Daniel Willard | 10,000 | All Fuel -10% |  |  |  |
| Dr. Robert Garbe | 6,000 | Steam Engine Purchase -15% |  |  |  |
| Dr. Wilhelm Schmidt | 22,000 | All Fuel -10% | Steam Traction +10% |  |  |
| Ernst Siemens | 6,000 | Electric Engine Purchase -30% |  |  |  |
| Eugen V. Debs | 65,000 | Company Overhead +25% | Goodwill +5% | Train Safety +25% | Passenger Revenue +10% |
| Fredrick Billings | 3,000 | Company Overhead -10% |  |  |  |
| George Nagelmachers | 46,000 | Car Maintenance +10% | Passenger Revenue +15% |  |  |
| George Pullman | 94,000 | Car Maintenance +20% | Passenger Revenue +25% | Station Revenue +5% |  |
| George Stephenson | 43,000 | All Engine Purchase -25% | Passenger Revenue +5% |  |  |
| George Westinghouse | 29,000 | Car Maintenance +5% | Train Safety +25% | Passenger Revenue +5% |  |
| Ginery Twitchell | 4,000 | Stock Price -15% | Goodwill +15% |  |  |
| Henri Giffard | 20,000 | Company Overhead +20% | Engine Maintenance -10% | Steam Engine Speed +5% |  |
| Henry Booth | 13,000 | Car Maintenance -10% |  |  |  |
| Herbert Garratt | 14,000 | Steam Fuel Costs -40% | All Engine Speed -5% |  |  |
| John Wootten | 4,000 | Engine Maintenance +10% | All Engine Fuel -5% | Steam Engine Fuel -25% |  |
| John Work Garrett | 21,000 | Track Cost -5% | Electric Track Building -40% | Track Grade Building -20% |  |
| Johns Hopkins | 41,000 | Passenger Revenue +10% |  |  |  |
| Oscar G. Murray | 87,000 | Station Turnaround Time 20% Faster | All Cargo Revenue +5% |  |  |
| Philip Randolph | 29,000 | Company Overhead +40% | Train Safety +10% | Passenger Revenue +10% |  |
| Richard Trevithick | 14,000 | All Engine Purchase -15% | All Engine Maintenance +15% | All Engine Speed +10 |  |
| Robert Gerwig | 37,000 | Bridge Building -40% | Track Building on Grade -50% | Electric Engine Acceleration +10% |  |
| Robert Stevens | 73,000 | All Track Building -25% | Track Maintenance -10% |  |  |
| Roy B. White | 12,000 | Station Building -20% | Track Building +15% | Station Turnaround Time 20% Faster |  |
| Rudolph Diesel | 3,000 | Diesel Engine Purchase -25% | Stock Prices -10% |  |  |
| Theodore D. Judah | 29,000 | Company Overhead -30% | Track Building -15% |  |  |
| Thomas Crampton | 4,000 | +10% Maintenance Track | All Engine Acceleration +10% | Steam Engine Speed +15% |  |
| Thomas Swann | 6,000 | Goodwill +10% |  |  |  |
| William C. VanHorne | 26,000 | Station Building -10% | Track Building -15% | Track Building on Grade -20% | Electric Engine Fuel -10% |
| William J. Palmer | 39,000 | Track Building -10% | Coal Revenue +30% |  |  |
| William Strong | 11,000 | Company Overhead +20% | Station Building -10% | Station Revenue +5% |  |

### Anexo H. Tendências dos chairmen (Tabela 3-1 do SG)

[SG > Cap. 3 > Table 3-1]. 100 = média; 200 = máximo. Colunas: Starts Company, Margin Buying, Short Selling, Investing in Others, Issue Bonds, Expand Rail, Industry Investment. Célula vazia = valor não impresso no HTML.

| Name | Starts Company | Margin Buying | Short Selling | Investing in Others | Issue Bonds | Expand Rail | Industry Investment |
|---|---|---|---|---|---|---|---|
| Attlee, Clement | 35 | 100 | 100 | 50 | 145 | 100 | 135 |
| Bismarck, Otto von | 20 | 100 | 100 | 120 | 150 | 30 | 175 |
| Bleichroder, Gerson von | 50 | 75 | 55 | 150 | 100 | 40 | 175 |
| Brunel, Isambard Kingdom | 200 | 25 | 25 | 100 | 175 | 200 | 80 |
| Crocker, Charles | 200 | 100 | 10 | 200 | 100 | 200 | 135 |
| Donnersmarck, Henckel von | 100 | 115 | 125 | 180 | 105 | 90 | 145 |
| Drew, Daniel | 140 | 180 | 180 | 150 | 85 | 100 | 35 |
| Durant, Thomas | 100 | 115 | 100 | 100 | 175 | 100 | 165 |
| Emperor Meiji | 140 | 100 | 100 | 100 | 100 | 100 | 100 |
| Fisk, James | 170 | 185 | 145 | 165 | 100 | 100 | 160 |
| Fleming, Sir Sanford | 100 | 100 | 100 | 120 | 200 | 100 | 20 |
| Forrest, Sir John | 125 | 85 | 75 | 100 | 140 | 175 | 40 |
| Fremont, John C. | 180 | 70 | 70 | 60 | 170 | 135 | 100 |
| General Gentaro | 25 | 30 | 30 | 100 | 100 | 180 | 100 |
| Gould, Jay | 20 | 200 | 200 | 200 | 90 | 55 | 110 |
| Hansemann, Adolph von | 45 | 200 | 100 | 155 | 100 | 65 | 150 |
| Harriman, Edward Henry | 170 | 185 | 190 | 160 | 120 | 180 | 185 |
| Hill, James | 200 | 100 | 100 | 50 | 200 | 200 | 140 |
| Hincks, Sir Francis | 100 | 100 | 100 | 100 | 100 | 120 | 75 |
| Holliday, Cyrus K. | 165 | 40 | 35 | 100 | 125 | 135 | 100 |
| Hopkins, Mark | 100 | 100 | 100 | 100 | 100 | 100 | 100 |
| Hudson, George | 135 | 200 | 200 | 120 | 140 | 70 | 100 |
| Huntington, Collis | 190 | 70 | 70 | 105 | 155 | 120 | 170 |
| Jay Cooke, | 80 | 115 | 100 | 150 | 200 | 100 | 100 |
| Kai-Shek, Chiang | 145 | 120 | 115 | 120 | 20 | 55 | 100 |
| Keith, Minor | 130 | 100 | 100 | 155 | 100 | 120 | 175 |
| Morgan, J.P. | 100 | 130 | 100 | 200 | 100 | 100 | 200 |
| Napoleon III | 100 | 100 | 100 | 100 | 100 | 145 | 160 |
| Nehru, Jawaharlal | 35 | 100 | 100 | 35 | 100 | 165 | 145 |
| Platner, George | 140 | 100 | 100 | 100 | 115 | 165 | 100 |
| Rhodes, Cecil | 200 | 100 | 100 | 100 | 100 | 200 | 155 |
| Scott, Thomas | 130 | 100 | 100 | 100 | 145 | 170 | 100 |
| Stanford, Leland | 100 | 100 | 40 | 185 | 120 | 100 | 140 |
| Stephen, Sir George | 100 | 100 | 100 | 100 | 100 | 100 | 50 |
| Strathcona, Lord | 100 | 100 | 100 | 100 | 100 | 100 | 20 |
| Thomas, Philip E. | 100 | 100 | 100 | 100 | 100 | 100 | 100 |
| Train, George F. | 160 | 100 | 100 | 100 | 100 | 100 | 200 |
| Vanderbilt, Cornelius | 100 | 100 | 100 | 100 | 100 | 100 | 190 |
| Wheelwright, William | 120 | 120 | 80 | 100 | 100 | 115 | 170 |
| Zedong, Mao | 20 | 20 | 20 | 100 | 140 | 145 | 100 |

### Anexo I. Melhorias de estação (Tabela 5-1 do SG)

[SG > Cap. 5 > Table 5-1]. Algumas células do HTML estão trocadas (linha "Telephone Poles"); a seção 3.3 segue o texto.

| Type | Upgrade | Year | Cost | Description |
|---|---|---|---|---|
| Station Size | Station, Medium | 1830* | $100,000 | Upgrade to Medium Station |
| Station Size | Station, Large | 1830 | $200,000 | Upgrade to Large Station |
| Reduction to Turnaround Time | Customs House | 1830 | $25,000 | Reduce customs delays by 50% |
| Reduction to Turnaround Time | Telegraph Poles | 1837 | $40,000 | Reduce station turnaround by 25% |
| Reduction to Turnaround Time turnaround by 50% | Telephone Poles | 1905 | $80,000 | Reduce station |
| Increase to Passenger Revenue | Saloon | 1830 | $16,000 | 10% increase to passenger revenue |
| Increase to Passenger Revenue | Restaurant, Small | 1830 | $50,000 | 20% increase to passenger revenue |
| Increase to Passenger Revenue | Restaurant, Large | 1850 | $100,000 | 30% increase to passenger revenue |
| Increase to Passenger Revenue | Hotel, Small | 1830 | $65,000 | 20% increase to passenger revenue |
| Increase to Passenger Revenue | Hotel, Large | 1878 | $175,000 | 40% increase to passenger revenue |
| Reduction to Pickup Delay Penalty | Warehouse | 1830 | $25,000 | 50% reduction in pickup delay penalty for goods, cotton and wool |
| Reduction to Pickup Delay Penalty | Post Office | 1840 | $35,000 | 50% reduction in pickup delay penalty for mail |
| Reduction to Pickup Delay Penalty | Refrigerated Storage | 1860 | $50,000 | 50% reduction in pickup delay penalty for produce and milk. |
| Reduction to Pickup Delay Penalty | Grain Silo | 1873 | $35,000 | 50% reduction in pickup delay penalty for grain, coffee, and sugar. |
| Reduction to Pickup Delay Penalty | Liquid Storage | 1885 | $50,000 | 50% reduction in pickup delay penalty for oil and chemicals. |
| Train Performance | Water Tower | 1830 | $25,000 | Refills water in steam engines. Steam engines without water travel much slower. |
| Train Performance | Sanding Tower | 1830 | $30,000 | Refills sand. Trains without sand travel 50% slower |
| Train Performance | Roundhouse | 1830 | $120,000 | Provides maintenance (oil). Low oil increases risk of breakdown /crash |

### Anexo J. Pontes (Tabela 5-3) e custo base de trilho (Tabela 5-4)

[SG > Cap. 5 > Table 5-3 e Table 5-4]. Custo por quadrado. Ver seção 2.

| Type | Year Available | Cost (on flat ground) | Special Restrictions * |  |
|---|---|---|---|---|
| Single track | Double track |  |  |  |
| Wooden | 1830 | $60,000 | n.a. | Only available in single track; trains slow down 40 percent while crossing |
| Stone | 1840 | $200,000 | $300,000 | None |
| Steel | 1880 | $150,000 | $225,000 | Trains slow down 20 percent while crossing |


| Base Track Costs (per square) | Non-electrified | Electrified |
|---|---|---|
| Single | $4,000 | $10,000 |
| Double | $6,000 | $15,000 |

### Anexo K. Rating de crédito: razão dívida/ativos (Tabela 5-5)

[SG > Cap. 5 > Table 5-5]. Ver seção 8.4. A Tabela 5-6 (cobertura de dívida) não aparece no HTML.

| Ratio | Credit Rating |
|---|---|
| 0 | 0.24 AAA |
| 0.25 | 0.37 AA |
| 0.38 | 0.49 A |
| 0.5 | 0.57 BBB |
| 0.58 | 0.66 BB |
| 0.67 | 0.74 B |
| 0.75 | 0.8 CCC |
| 0.81 | 0.86 CC |
| 0.87 | 0.95 C |
| 0.95 or worse | F |

### Anexo L. Confiabilidade x quebras (Tabela 4-1 do SG)

[SG > Cap. 4 > Table 4-1]. Ver seção 4.4.

| Train Rating Reliability | Cells Traveled Between Breakdowns (Average) |
|---|---|
| Atrocious | 400 |
| Extremely Poor | 600 |
| Very Poor | 875 |
| Poor | 1,100 |
| Below Average | 1,400 |
| Average | 1,800 |
| Above Average | 2,500 |
| Good | 4,000 |
| Very Good | 6,000 |
| Outstanding | 12,000 |
| Near Perfect | 24,000 |

### Anexo M. Tamanho populacional e aparência (Tabela 11 do SG)

[SG > Cap. 2 > Table 11 (aparência de cidades)]. Ver seção 6.3.

| Size | Appearance |
|---|---|
| Village | 1 to 2 buildings are likely; if an industry is set for this population center, few houses will show up with it, if any. |
| Town | 1 to 4 buildings are likely; usually no more that 2 houses are likely to appear if an industry is set for this population center. |
| City | 4 to 8 buildings are likely; any combination of industries and housing will appear. |
| Metropolis | 6 to 11 buildings are likely; any combination of industries and housing will appear. |
| Megopolis | 8+ buildings are likely; any combination of industries and housing will appear |

### Anexo N. Metas de cenário: campanha RT2 (The Iron Seed e 17 cenários)

Fonte: [SG > Cap. 6]. Metas cumulativas (ver seção 10.4).

Fonte: Strategy Guide.html, cap. 6 "The Lay of the Land" (s.txt linhas 2796-3250, blocos "Award Objectives" e "Available Bonuses"). Títulos atribuídos pela ordem dos blocos (ver linhas de título em s.txt).

| # | Cenário | Bronze | Silver | Gold | Bônus disponíveis (escolha ao iniciar) |
|---|---|---|---|---|---|
| 1 | The Iron Seed | Conectar Baltimore a Washington e transportar 4 cargas até o fim de 1840 | Além do anterior: conectar a Harper's Ferry e transportar pelo menos 8 cargas | Além de tudo: conectar a Philadelphia e transportar pelo menos 12 cargas | +$50.000; 10% menor custo de trilho; John Bull 2-4-0 disponível cedo |
| 2 | A Handle on the Bread Basket | Conectar Nova York e Chicago e transportar 8 cargas entre elas até 1870 | Igual, com equity (book value) da empresa >= $4M | Tudo acima, até 1858 | +$80.000; 15% menor custo de trilho; +20% receita de aço |
| 3 | Bridging a Nation | Conectar as cidades escolhidas e transportar 6 cargas entre elas até 1882 | Conectar até 1882 e equity de $10 milhões | Conectar até 1878 e patrimônio pessoal de $8 milhões | Rotas: St. Louis-Sacramento; St. Paul-Seattle; New Orleans-Los Angeles (não são bônus) |
| 4 | Silver Booms and Market Busts | Patrimônio pessoal >= $5 milhões até 1905 | Patrimônio pessoal $10 milhões até 1900 | Patrimônio pessoal $20 milhões até 1895 | +20% preço de ações, 15% menor custo de trilho; +20% segurança, +10% velocidade do trem; Consolidation 2-8-0 cedo, +15% receita de gado |
| 5 | Whistle Stops and Promises | Levar Teddy a 13 cidades até meia-noite de 14 de janeiro | 14 cidades | 16 cidades | Atlantic 4-4-2 (alta velocidade, péssimo em grades); Mogul 2-6-0 (média/média); Camelback 2-6-0 (baixa velocidade, excelente em grades) |
| 6 | Crossing the Great Divide | Conectar Halifax a Vancouver e transportar 6 cargas entre elas até 1896 | Acima + equity > $20 milhões até 1896 | Acima + patrimônio da empresa > $50 milhões até 1896 | Manager Van Horne (10% menor custo de trilho); Consolidation cedo, +10% velocidade; um nível a mais de rating de crédito, 10% mais rápido na estação |
| 7 | Birth of the Iron Horse | Maior patrimônio pessoal no fim de 1855 | Maior patrimônio pessoal e > $10 milhões até 1855 | Patrimônio pessoal >= $20 milhões até 1885 e ser a única ferrovia sobrevivente na Grã-Bretanha | John Bull cedo, +10% velocidade; +25% receita de carvão, +$50.000; um nível de rating de crédito, +30% preço de ação |
| 8 | Excess on the Orient Express | Fazer a conexão até o fim de 1889 e transportar 24 cargas entre as cidades | Concluir até 1887 | Concluir até 1883 | Manager Robert Gerwig (custo de combustível a vapor 25% menor); +25% receita de correio e passageiros; Manager Naglemachers (um nível de rating a mais, +20% goodwill) |
| 9 | Knitting with Iron | 8 estados conectados até o fim de 1876 | 9 a 10 estados até o fim de 1875 | 11+ estados, incluindo Baviera, e 70 cargas transportadas em um ano até o fim de 1874 | Manager Robert Gerwig (20% menor custo de ponte); +$80.000, Iron Duke cedo; +$100.000, +10% goodwill |
| 10 | Next Stop: The 20th Century | Investimentos totais em indústrias >= $10 milhões até 1885 | Acima + pelo menos um ano com lucro de indústrias > $1 milhão | Completar ambos até 1875 | Shay 3-Truck cedo; Manager Thomas Crampton (+15% velocidade de motor); +$80.000, +25% goodwill |
| 11 | Brenner Pass | Conectar Munique a Verona e transportar 12 cargas (car loads) entre elas até 1878 | Até 1875, com 24 cargas | Conexão até 1872, com 32 cargas | Manager Robert Gerwig (+30% combustível a vapor); +25% velocidade de motor; +$80.000, custo de trilho em montanha -10% |
| 12 | When Walls Come Down | Patrimônio pessoal de $5 milhões e conectar Aalborg (Dinamarca) a Varsóvia até o fim de 2020 | Acima + conectar Veneza e patrimônio de $10 milhões | Acima + Istambul e patrimônio de $20 milhões | -20% custo de construção de trilho; -25% turnaround de estação; +20% velocidade de motor |
| 13 | Croissants or Crumpets | $10 milhões em investimentos em indústrias e conectar Panaji a Delhi e Pondicherry até 1890 | Acima + conectar Calcutá até 1880 | Acima + conectar Cabul e ser a única empresa restante até 1880 | -20% custo de trilho, Shay 3-Truck cedo; +$100.000, +15% goodwill; -25% overhead da empresa, +25% goodwill |
| 14 | The Samurai Rides an Iron Horse | Conectar Tóquio a Niigata, Nagoia e Sendai até 1899 | Acima + Sapporo e Quioto | Acima + Kagoshima e 50 cargas transportadas em um único ano | Shay 3-Truck cedo; +$50.000; +10% velocidade de motor |
| 15 | Which Way to the Coast? | Patrimônio pessoal de $10 milhões até 1958 | Até 1948 | Até 1938 | -20% tempo de turnaround de estação; +15% receita de passageiros e correio; +15% velocidade de motor |
| 16 | The People's Train | Equity (book value) de $10 milhões até 1972 | $20 milhões até 1967 | $40 milhões até 1962 | -20% manutenção de trilho; +10% receita de passageiros e correio; +20% lucro de estação |
| 17 | Dilemma Down Under | Lucro anual de $1 milhão por 3 anos seguidos até o fim de 1982 | Até o fim de 1977 | Até o fim de 1972 | +1 rating de crédito; -25% compra de trem; -20% custo de combustível |
| 18 | Cape to Cairo | Conectar Cidade do Cabo a Cairo e transportar pelo menos 8 cargas entre elas até 1930 | Até 1916 | Até 1902 | +$70.000, +1 rating de crédito; +10% receita de passageiros, -20% turnaround de estação; +15% velocidade de trem |

Notas:
- "Britain", "Central Europe", "China" etc. (abaixo) são cenários avulsos (Stand Alone), não campanha.
- Na tabela original, cenário 11 (Brenner Pass) diz "12 car loads" no Bronze; o Gold usa "32 car loads".
- Cenário 12: o texto original diz "connect Alborg, Denmark, to Warsaw by the end of 2020".

### Anexo O. Metas de cenário: avulsos (12)

Fonte: [SG > Cap. 6]. Os 12 cenários estão na ordem de aparição no guia.

Fonte: Strategy Guide.html, cap. 6 (s.txt linhas ~3284-3440). Cenários avulsos (12). "Going for the Gold" = texto de dica do guia; o texto original não traz bônus disponíveis para estes.

| Cenário | Bronze | Silver | Gold |
|---|---|---|---|
| Britain | Patrimônio pessoal $5 milhões até 1899 | Patrimônio pessoal $15 milhões até 1899 | Patrimônio pessoal $40 milhões até 1899 |
| Central Europe | Acesso a todas as 6 cidades-hub em 25 anos | Acima + patrimônio pessoal de $15 milhões | Acima + equity da empresa de $40 milhões em 25 anos |
| China | Patrimônio pessoal $7 milhões em 35 anos | Patrimônio pessoal $20 milhões em 35 anos | Patrimônio pessoal $35 milhões em 35 anos + conectar Pequim a Zhanjiang |
| Eastern USA | Maior patrimônio pessoal em 25 anos | Acima + mais cargas transportadas | Acima + equity de $40 milhões |
| Korea | Equity da empresa de $20 milhões até 1970 | Acima + conectar Busan a Chongjin | Acima + transportar 80 cargas em um ano |
| North America | (sem objetivo) | (sem objetivo) | (sem objetivo) |
| Scandinavia | Equity de $15 milhões em 25 anos | Acima + conectar Noruega à Alemanha | Acima + transportar 100 car loads em um ano |
| South America | Maior patrimônio pessoal após 30 anos | Acima, com patrimônio > $20 milhões | Acima, com patrimônio > $50 milhões |
| Southern USA | Maior patrimônio pessoal no fim de 30 anos | Acima + equity de pelo menos $20 milhões | Acima + controlar pelo menos 70% da ferrovia existente no mapa |
| The Mediterranean | Patrimônio pessoal $5 milhões em 25 anos | Acima + maior número de cargas transportadas (lifetime) | Equity de pelo menos $55 milhões, além do anterior |
| United States | Patrimônio pessoal $10 milhões em 35 anos | Acima + equity de $30 milhões | Patrimônio pessoal e equity de pelo menos $50 milhões |
| Western USA | Patrimônio de $5 milhões em 20 anos | Acima + patrimônio da empresa de $40 milhões | Acima + transportar 120 cargas em um ano |

Nota: a lista de bônus desses cenários avulsos não aparece nos blocos extraídos; verificar no texto completo se necessário.

### Anexo P. Funções do editor de eventos (Appendix A de E3)

Fonte: [E3 > Appendix A]. 84 entradas. "Optional - Class" = aceita IsHighestInClass / IsLowestInClass.

| Função | Parâmetro | Descrição (texto original, resumido por espaço) |
|---|---|---|
| Random1To10000 | None | Generates a random number between 1 and 10000 |
| GameNumPlayers | None | The number of computer and human players actively in the game. (Basically, the number of players the human chose to have in the game on the setup screen) |
| GameNumCompanies | None | The number of active companies in the game. Liquidated companies don't count |
| GameExistsCompanyId | Required (ID) | True if the company with the ID specified in the parameter is actively in the game. |
| GameExistsPlayerId | Required (ID) | True if the player with the ID specified in the parameter is actively in the game |
| GameYearsElapsedSinceStart | None | The number of years elapsed in the game. For the first 12 months, this evaluates to 0, for the next 12 months, to 1, etc. |
| GameStartYear | None | The year this game started. |
| GameYear | None | The current year the game is in. |
| GameMonth | None | The current month the game is in. |
| GameYearMonth | None | The current year and month the game is in, handily combined into a single variable - if the current date was July, 1865, this would evaluate to 186507 |
| GameEconomicState | None | The current game economy, ranges from 0 to 4, with 0 being depression and 4 being boom times |
| GameLoadsHauledThisYear | None | The number of car loads hauled by all companies this year. Empty cars don't count. The distance hauled does not matter for the purposes of this and similar functions. |
| GameLoadsHauledLifetime | None | The number of car loads hauled by all companies since the beginning of the game. |
| GameTrackCells | None | The number of cells of track in existence from all companies. |
| GameSingleTrackCells | None | The number of cells of single track. |
| GameDoubleTrackCells | None | The number of cells of double track. |
| GameElectricTrackCells | None | The number of cells of electric track. |
| GameVariable1 | None | A wildcard variable you can set up as a result of one event that can affect the triggers of other events |
| GameVariable2 | None | A wildcard variable |
| GameVariable3 | None | A wildcard variable |
| GameVariable4 | None | A wildcard variable |
| CompanyCash | Optional - Class | The cash on hand of a company. |
| CompanyDebt | Optional - Class | The total debt of a company (All bonds, plus negative cash, if cash is negative) For a company with one $500,000 bond and cash of -$34,000, this would evaluate to 534000 |
| CompanyTotalAssets | Optional - Class | Total assets of a company, including cash (if positive), track, trains, etc.. Debt is not subtracted from this number. To get net assets, subtract CompanyDebt from CompanyTotalAssets |
| CompanyBookValue | Optional - Class | Company total assets minus debt i.e. the shareholder’s equity |
| CompanyRevenueThisYear | Optional - Class | Company’s revenue for this year only. |
| CompanyRevenueLifetime | Optional - Class | Company’s revenue for its entire lifetime. |
| CompanyNetIncomeThisYear | Optional - Class | Company’s net income for this year only |
| CompanyNetIncomeLifetime | Optional - Class | Company’s net income for its entire lifetime |
| CompanyLoadsHauledThisYear | Optional - Class | Number of car loads hauled this year by this company. |
| CompanyLoadsHauledLifetime | Optional - Class | Number of car loads hauled in lifetime by this company |
| CompanyTrackCells | Optional - Class | The number of cells of track owned by this company. |
| CompanySingleTrackCells | Optional - Class | The number of cells of single track owned by this company. |
| CompanyDoubleTrackCells | Optional - Class | The number of cells of double track owned by this company. |
| CompanyElectricTrackCells | Optional - Class | The number of cells of electric track owned by this company. |
| CompanyGoodwill | Optional - Class | The goodwill of this company |
| CompanyId | Optional - Class | The ID of this company. Each company has a unique ID that never changes. In the editor, you can see company IDs on the listbox at the bottom of the main screen. They are the number just to the left of the company name i.e. for 2) Union Pacific the id of that company is 2. |
| CompanyVariable1 | Optional - Class | A wildcard variable you can set up as a result of one event that can affect the triggers of other events. A unique set of these variables is saved for each company. |
| CompanyVariable2 | Optional - Class | A wildcard variable |
| CompanyVariable3 | Optional - Class | A wildcard variable |
| CompanyVariable4 | Optional - Class | A wildcard variable |
| CompanyTerritoryConnectedToTerritory | Required (ID) | Pass in the ID of the territory you want to test against. True if there is a station controlled by the current company within each territory that can trace a continuous route of track to the other territory. Track ownership does not matter. |
| CompanyTerritoryConnectedToCity | Required (ID) | Pass in the ID of the city you want to test against. True if there is a station controlled by the current company within the current territory and the target city that can trace a continuous route of track to the other territory. Track ownership does not matter. |
| CompanyTerritoryGoodwill | Optional - Class | The goodwill of a company in the current territory. |
| CompanyTerritoryHasTrainAccess | Optional - Class | True if the company can run trains through the current territory. |
| CompanyTerritoryHasTrackAccess | Optional - Class | True if the company can lay track through the current territory. |
| CompanyTerritoryHasStationAccess | Optional - Class | True if the company can build stations in the current territory. |
| CompanyTerritoryHasBuildingBuyAccess | Optional - Class | True if the company can buy buildings in the current territory. |
| CompanyTerritoryHasAnyAccess | Optional - Class | True if the company can do any of the above 4 items. |
| CompanyTerritoryTrackCells | Optional - Class | The number of cells of track the company has in the current territory. |
| CompanyTerritoryLoadsHauledThisYear | Optional - Class | The number of car loads hauled this year TO (not from) the current territory. |
| CompanyTerritoryLoadsHauledLifetimeFromTerritory | Optional - Class | The number of car loads hauled lifetime to specified territory from a specified territory. |
| CompanyTerritoryLoadsHauledBetweenTerritory | Optional - Class | The number of car loads hauled between to specified territories |
| CompanyTerritoryLoadsHauledLifetime | Optional - Class | The number of car loads hauled lifetime TO (not from) the current territory. |
| PlayerCash | Optional - Class | Cash for the current player. |
| PlayerStockValue | Optional - Class | Value of stock holdings for the current player. |
| PlayerNetWorth | Optional - Class | Total net worth for the current player - the sum of the above 2 items. |
| PlayerControlsAnyCompany | Optional - Class | True if the player is currently chairman of any company |
| PlayerControlledCompanyId | Optional - Class | Returns the ID of the company the player currently controls (0 if the player controls no company) |
| PlayerId | Optional - Class | The ID of this player. Each player has a unique ID that never changes. In the editor, you can see player IDs on the listbox at the bottom of the main screen. They are the number just to the left of the player name i.e. for 3) Jay Gould - that player's ID is 3 |
| PlayerVariable1 | Optional - Class | A wildcard variable you can set up as a result of one event that can affect the triggers of other events. A unique set of these variables is saved for each player. |
| PlayerVariable2 | Optional - Class | A wildcard variable |
| PlayerVariable3 | Optional - Class | A wildcard variable |
| PlayerVariable4 | Optional - Class | A wildcard variable |
| TerritoryTrackCells | Optional - Class | The number of cells of track from all companies in the current territory |
| TerritoryLoadsHauledThisYear | Optional - Class | The number of car loads hauled by all companies TO this territory this year. (Loads hauled FROM the territory don't count) |
| TerritoryLoadsHauledLifetime | Optional - Class | The number of car loads hauled by all companies TO this territory since the start of the game. (Loads hauled FROM the territory don't count) |
| TerritoryConnectedToTerritory | Required (ID) | Pass in the ID of the territory you want to test against. True if there is a station within each territory that can trace a continuous route of track to the other territory, regardless of which company owns the track or stations |
| TerritoryConnectedToCity | Required (ID) | Pass in the ID of the city you want to test against. True if there is a station within the current territory and the target city that can trace a continuous route of track to the other territory, regardless of which company owns the track or stations. |
| TerritoryId | Optional - Class | The ID of this player. Each territory has a unique ID that never changes. In the editor, you can see territory IDs on the listbox at the bottom of the main screen, as you paint territories. They are the number just to the left of the player name i.e. for 3) Germany - that territory's ID is 3 |
| TerritoryVariable1 | Optional - Class | A wildcard variable you can set up as a result of one event that can affect the triggers of other events. A unique set of these variables is saved for each territory. |
| TerritoryVariable2 | Optional - Class | A wildcard variable |
| TerritoryVariable3 | Optional - Class | A wildcard variable |
| TerritoryVariable4 | Optional - Class | A wildcard variable |
| True | None | Evaluates to true of course |
| False | None | Evaluates to false (False is represented internally as the number 0) |
| CompanyTerritoryLoadsHauledLifetimeFromTerritory | Mandatory - TerritoryId | Used to determine how many loads have been been hauled from the territory passed in as a parameter to the current territory being evaluated |
| CompanyTerritoryLoadsHauledLifetimeBetweenTerritory | Mandatory - TerritoryId | Used to determine how many loads have been been hauled between the territory passed in as a parameter and the current territory being evaluated |
| CampaignChoice1To3 | None | Used to set up campaign maps - refers to the choice the player made on the main campaign screen. |
| CampaignDifficulty1To3 | None | Used to set up campaign maps - refers to the overall difficulty the player is playing the campaign at. |
| StationVisited | Mandatory - StationId | True if the specific station identified by StationId has been visited by a train by any player in the game. |
| NumberOfStationsVisited | None | The number of different stations visited by trains in this game. |
| CompanyIndustryProfitsThisYear | Optional - Class | The industry profits by this company this year. |
| CompanyIndustryInvestments | Optional - Class | The total level of industry investments by this company. |

### Anexo Q. Efeitos do editor de eventos (Appendix B de E3)

Fonte: [E3 > Appendix B] e [AG > Apêndice B]. 167 entradas. Os efeitos de variável (Global, Company, Player, Territory, 1 a 4) estão no fim da lista. Os efeitos do TSC e as listas de triggers e tipos de carga estão na seção 10.2.

| Efeito | Tipo / faixa | Descrição |
|---|---|---|
| Game Over - Win 1 (instant) | on/off | This is the bronze finish. |
| Game Over - Win 2 (instant) | on/off | This is the silver finish. |
| Game Over - Win 3 (instant) | on/off | This is the gold finish. |
| Game Over - Loss (instant) | on/off | This is the loss trigger. |
| Company Cash (instant) | Variable $ |  |
| Player Cash (instant) | Variable $ |  |
| Economic Status (instant) | -4 to +4 levels | 4 being the best, -4 the worst. |
| Acceleration | Variable %  -99 to +100 |  |
| Acceleration - Diesel | Variable %  -99 to +100 |  |
| Acceleration - Electric | Variable %  -99 to +100 |  |
| Acceleration -Steam | Variable %  -99 to +100 |  |
| Bridge Building | Variable %  -99 to +100 |  |
| Building Buy Cost | Variable %  -99 to +100 |  |
| Company Overhead | Variable %  -99 to +100 |  |
| Credit Rating | -10 to +10 levels | 10 being the best, -10 the worst. |
| Diesel Engine Purchase | Variable %  -99 to +100 |  |
| Electric Engine Purchase | Variable %  -99 to +100 |  |
| Electric Track Building | Variable %  -99 to +100 |  |
| Engine Maintenance | Variable %  -99 to +100 |  |
| Engine Maintenance- Diesel | Variable %  -99 to +100 |  |
| Engine Maintenance- Electric | Variable %  -99 to +100 |  |
| Engine Maintenance- Steam | Variable %  -99 to +100 |  |
| Engine Purchase | Variable %  -99 to +100 |  |
| Fuel Costs | Variable %  -99 to +100 |  |
| Fuel Costs - Diesel | Variable %  -99 to +100 |  |
| Fuel Costs - Electric | Variable %  -99 to +100 |  |
| Fuel Costs - Steam | Variable %  -99 to +100 |  |
| Manager Salaries | Variable %  -99 to +100 |  |
| Merger Premium | Variable %  -99 to +100 |  |
| Mountainous Track Building | Variable %  -99 to +100 |  |
| Political Savvy | Variable %  -99 to +100 |  |
| Prime Rate | -5 to +5 levels | 5 being the best, -5 the worst. |
| Station Building | Variable %  -99 to +100 |  |
| Station Turnaround | Variable %  -99 to +100 |  |
| Steam Engine Purchase | Variable %  -99 to +100 |  |
| Stock Prices | Variable %  -99 to +100 |  |
| Track Building | Variable %  -99 to +100 |  |
| Track Maintenance | Variable %  -99 to +100 |  |
| Traction | Variable %  -99 to +100 |  |
| Traction - Diesel | Variable %  -99 to +100 |  |
| Traction - Electric | Variable %  -99 to +100 |  |
| Traction - Steam | Variable %  -99 to +100 |  |
| Train Safety | Variable %  -99 to +100 |  |
| Train Speed | Variable %  -99 to +100 |  |
| Train Speed - Diesel | Variable %  -99 to +100 |  |
| Train Speed - Electric | Variable %  -99 to +100 |  |
| Train Speed - Steam | Variable %  -99 to +100 |  |
| Territory Allow all | All or Nothing |  |
| Territory Allow Track Build | All or Nothing |  |
| Territory Allow Train Run | All or Nothing |  |
| Territory Allow Station Build | All or Nothing |  |
| Territory Allow Building Buy | All or Nothing |  |
| Territory Confiscate All (instant) | None or All |  |
| Territory Confiscate Track (instant) | None or All |  |
| Territory Confiscate Trains (instant) | None or All |  |
| Territory Confiscate Stations(instant) | None or All |  |
| Territory Confiscate Buildings (instant) | None or All |  |
| All Passenger & Cargo Production | Variable %  -99 to +100 |  |
| All Passenger & Cargo Revenue | Variable %  -99 to +100 |  |
| Aluminum Revenue | Variable %  -99 to +100 |  |
| Aluminum Revenue | Variable %  -99 to +100 |  |
| Auto Production | Variable %  -99 to +100 |  |
| Auto Revenue | Variable %  -99 to +100 |  |
| Bauxite Production | Variable %  -99 to +100 |  |
| Bauxite Revenue | Variable %  -99 to +100 |  |
| Cattle Production | Variable %  -99 to +100 |  |
| Cattle Revenue | Variable %  -99 to +100 |  |
| Cement Production | Variable %  -99 to +100 |  |
| Cement Revenue | Variable %  -99 to +100 |  |
| Chemical Production | Variable %  -99 to +100 |  |
| Chemical Revenue | Variable %  -99 to +100 |  |
| Coal Revenue | Variable %  -99 to +100 |  |
| Coal Revenue | Variable %  -99 to +100 |  |
| Coffee Revenue | Variable %  -99 to +100 |  |
| Coffee Revenue | Variable %  -99 to +100 |  |
| Cotton Production | Variable %  -99 to +100 |  |
| Cotton Revenue | Variable %  -99 to +100 |  |
| Diesel Production | Variable %  -99 to +100 |  |
| Diesel Revenue | Variable %  -99 to +100 |  |
| Fertilizer Production | Variable %  -99 to +100 |  |
| Fertilizer Revenue | Variable %  -99 to +100 |  |
| Food Production | Variable %  -99 to +100 |  |
| Food Revenue | Variable %  -99 to +100 |  |
| Goods Production | Variable %  -99 to +100 |  |
| Goods Revenue | Variable %  -99 to +100 |  |
| Grain Revenue | Variable %  -99 to +100 |  |
| Grain Revenue | Variable %  -99 to +100 |  |
| Gravel Production | Variable %  -99 to +100 |  |
| Gravel Revenue | Variable %  -99 to +100 |  |
| Iron Production | Variable %  -99 to +100 |  |
| Iron Revenue | Variable %  -99 to +100 |  |
| Log Production | Variable %  -99 to +100 |  |
| Log Revenue | Variable %  -99 to +100 |  |
| Lumber Production | Variable %  -99 to +100 |  |
| Lumber Revenue | Variable %  -99 to +100 |  |
| Mail Production | Variable %  -99 to +100 |  |
| Mail Revenue | Variable %  -99 to +100 |  |
| Milk Production | Variable %  -99 to +100 |  |
| Milk Revenue | Variable %  -99 to +100 |  |
| Oil Production | Variable %  -99 to +100 |  |
| Oil Revenue | Variable %  -99 to +100 |  |
| Paper Production | Variable %  -99 to +100 |  |
| Paper Revenue | Variable %  -99 to +100 |  |
| Passenger Production | Variable %  -99 to +100 |  |
| Passenger Revenue | Variable %  -99 to +100 |  |
| Produce Production | Variable %  -99 to +100 |  |
| Produce Revenue | Variable %  -99 to +100 |  |
| Pulpwood Production | Variable %  -99 to +100 |  |
| Pulpwood Revenue | Variable %  -99 to +100 |  |
| Rubber Production | Variable %  -99 to +100 |  |
| Rubber Revenue | Variable %  -99 to +100 |  |
| Station Production | Variable %  -99 to +100 |  |
| Station Revenue | Variable %  -99 to +100 |  |
| Steel Production | Variable %  -99 to +100 |  |
| Steel Revenue | Variable %  -99 to +100 |  |
| Sugar Production | Variable %  -99 to +100 |  |
| Sugar Revenue | Variable %  -99 to +100 |  |
| Tire Production | Variable %  -99 to +100 |  |
| Tire Revenue | Variable %  -99 to +100 |  |
| Uranium Production | Variable %  -99 to +100 |  |
| Uranium Revenue | Variable %  -99 to +100 |  |
| Waste Revenue | Variable %  -99 to +100 |  |
| Waste Revenue | Variable %  -99 to +100 |  |
| Wool Production | Variable %  -99 to +100 |  |
| Wool Revenue | Variable %  -99 to +100 |  |
| (Locomotive Name) Availability | Normal,  IMMEDIATE,  NEVER | Any locomotive can be triggered. |
| (Manager Name) Availability | Normal,  IMMEDIATE, NEVER | Any manager can be triggered. |
| Set Global Variable1(GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Global Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Global Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Global Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add  To Global Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Global Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Global Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Global Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Company Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Company Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Company Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Company Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Company Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Company Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Company Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Company Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Player Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Player Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Player Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Player Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Player Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Player Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Player Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Player Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Territory Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Territory Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Territory Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Set Territory Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Territory Variable1 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Territory Variable2 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Territory Variable3 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| Add To Territory Variable4 (GAMEVARIABLE) | Variable # -99 to +100 | Numbers can be used to add and subtract. |
| ****** Additional features introduced in RT2 - The Second Century ****** | ****** EVENT TRIGGERS ****** | The following expressions may now be used as triggers for events: |
| CompanyIndustryProfitsLifetime | CompanyTerritoryLoadsHauledFromThisYear | CompanyTerritoryLoadsHauledFromLifetime CompanyTerritoryLoadsHauledFromLifetimeToTerritory CompanyTerritoryTypeLoadsHauledToThisYear CompanyTerritoryTypeLoadsHauledToLifetime CompanyTerritoryTypeLoadsHauledFromThisYear CompanyTerritoryTypeLoadsHauledFromLifetime CompanyTypeLoadsHauledThisYear CompanyTypeLoadsHauledLifetime CompanyAverageSpeedThisYear CompanyAverageSpeedLifetime GameTrackCellsVisited TerritoryCompletelyFlooded CompanyTrackCellsRemaining CompanyHasAnyAccessTerritoryParameter |
| For all the expressions with Type in them (CompanyType... and | CompanyTerritoryType...) you must provide a numeric cargo type as a | parameter, from the following list: |
| PASSENGERS        0 | MAIL              1 | LUMBER            2 LOGS              3 MILK              4 CATTLE            5 STEEL             6 GOODS             7 AUTOMOBILE        8 CEMENT            9 FOOD              10 COTTON            11 WOOL              12 PAPER             13 OIL               14 CHEMICALS         15 FERTILIZER        16 HAZARDOUS         17 COAL              18 GRAIN             19 ALUMINUM          20 COFFEE            21 TIRES             22 SUGAR             23 URANIUM           24 PULPWOOD          25 GRAVEL            26 IRON              27 PRODUCE           28 BAUXITE           29 DIESEL            30 RUBBER            31 TROOPS            32 AMMO              33 WEAPONS           34 ALCOHOL           35 METRA_RES_TO_COM  36 METRA_RES_TO_IND  37 METRA_ANY_TO_RES  38 METRA_RES_TO_AIR  39 METRA_COM_TO_AIR  40 METRA_AIR_TO_COM  41 |
| So, to determine if a company has hauled at least 20 loads of | coal in its lifetime, the syntax would be: |  |
|  | | CompanyTypeLoadsHauledLifetime(18) >= 20 |
|  | | ****** EVENT TEXT ****** |
| The text section of an event can now translate the following literals | into values when the event is displayed: |  |
| (PLAYER)         - The player for whom this event has been triggered | (COMPANY)        - The company for which this event has been triggered | (TERRITORY)      - The company for which this event has been triggered (TRIGGER)        - The result of the trigger for this event, formatted as a normal number (i.e. 2,421) (TRIGGER_MONEY)  - The result of the trigger for this event, formatted as a currency (i.e. $2,421) (TRIGGER_TF)     - The result of the trigger for this event, formatted as either true or false - any result other than 0 is considered true. |
| An example might be an event that shows how much coal a company has | hauled, with a trigger of |  |
|  | | CompanyTypeLoadsHauledLifetime(18) |
|  | | and message text of the following |
|  | | (COMPANY) has hauled (TRIGGER) loads of coal |
