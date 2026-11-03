### CrawlForge — コマンドラインのメッセージ(日本語)。
###
### English (locales/en/) is the source and the authority; every other folder is a
### translation of it. CONTRIBUTING.md, "Adding or improving a language", explains the rules.
### The comments above each message are context for translators: they are not shown.
###
### Japanese notes: CLDR "ja" has only the "other" plural category, so no selectors.
### Polite form (です/ます) in sentences; noun phrases for labels and table headers.

## Severity names

# Severity with an initial capital, for report headings.
severity_label_critical = 重大
severity_label_high = 高
severity_label_medium = 中
severity_label_low = 低
severity_label_info = 情報
# Severity in lower case, for table columns.
severity_word_critical = 重大
severity_word_high = 高
severity_word_medium = 中
severity_word_low = 低
severity_word_info = 情報

## Resumen del rastreo (`report.rs`)

crawl_finished = クロール完了: { $urls } URL、{ $secs } 秒
crawl_failed_suffix = {" "}({ $n } 件失敗)
crawl_truncated = クロールは { $reason } により打ち切られました。表示している問題は、その時点までに検出されたものです。
# A 429 is not a page's status: the site asked the crawler to slow down. The pages
# were left pending, and the useful thing to say is how to get them.
crawl_rate_limited_note = サイトがクロールのレートを制限しました。{ $n } ページは再試行後も 429 Too Many Requests を返したため、リンク切れとしては報告せず、保留にしています。サイト全体を必要とするルールは、これらのページを取得するまで評価されません。後で次を実行してください:
    {"    "}crawlforge resume { $path }
    または --concurrency を下げて再度クロールするか、サイトのファイアウォールで CrawlForge のユーザーエージェントを許可してください。
report_rate_limited_note = > **サイトがクロールのレートを制限しました。** { $n } ページが 429 Too Many Requests を返し、保留になっています。これらは集計にも含まれず、リンク切れとしても報告されません。サイト全体を必要とするルールも評価されていません。`crawlforge resume` で取得できます。
crawl_list_mode_note = リストクロール: 指定した URL のみを監査しました。サイト全体のリンク構造を必要とするルール(孤立ページ、被リンク、階層)は評価されません。
external_unchecked = { $n } 件の外部リンクを確認していません。max_external の上限に達しました。上限は --max-external で引き上げられます。
external_unregistered = { $n } 件の外部リンクは記録すらされていません。max_external_urls の上限に達しました。この上限は、1 ページから数十万のホストにリンクしているような場合のためのものです。
external_never_checked = { $n } 件の外部リンクは記録されましたが、一度も確認されていません。
external_offline = { $n } 件の外部リンクを確認していません。フォルダの監査ではネットワークにリクエストを送りません。確認するには、公開中のサイトを `crawlforge crawl <URL>` でクロールしてください。
external_check_disabled = 外部リンクは確認していません(--no-external-check)。このレポートに外部リンク切れがなくても、存在しないとは限りません。
external_checked_note = { $n } 件の外部リンクを確認しました。
rules_not_evaluated = サイト全体のリンク構造を必要とするルールは評価されていません: { $rules }。
results_title = 結果
label_urls = URL
label_html_pages = HTML ページ
label_links = リンク
note_internal = 内部 { $n }
note_indexable = インデックス可能 { $n }
heading_status_codes = ステータスコード
label_no_response = 応答なし
note_request_failed = リクエストに失敗
label_never_requested = 未リクエスト
note_never_requested = (検出済みだが、取得前にクロールが停止)
label_out_of_scope = 対象範囲外
note_out_of_scope = (監査したページからリンクされているが、このクロールの対象範囲外)
heading_non_indexable = インデックス不可の理由
heading_findings = 問題
no_findings = 問題はありません。
heading_excluded = 除外
# One collapsed template defect or several. $k is the number of distinct template defects,
# $pages the pages they cover.
template_issues = テンプレートの問題 { $k } 件({ $pages } ページ)
plus_more_findings = {" "}+ほか { $n } 件の問題
example_urls = 例: { $urls }

## Reformulación de reglas dominantes

pervasive_note = サイトの { $pct }%
deep_pages_summary = { $max } クリックより深いページが { $pages } 件 — サイトの { $pct }%(一般的な階層 { $low }–{ $high }、最深 { $deepest })
rule_deep_sorted = クリック階層の深い順に並べています。最初の列はトップページからのクリック数です。
hint_full_lists = ルールごとの全 URL リスト: { $cmd }

## Listado de una regla (`report --rule`)

rule_affected_urls = 該当 URL { $n } 件
rule_template_group = テンプレートグループ — { $n } ページ
rule_group_cause = 原因: { $detail }
rule_other_pages = その他の該当ページ — { $n }
rule_no_findings = このクロールでは { $rule } の問題はありません。
hint_who_links = リンク切れは、リンク元のページで修正します。ある URL へのリンク元とそのアンカーテキストを確認するには:
error_unknown_rule = { $rule } はルール ID ではありません。ルール一覧は `crawlforge rules` で表示できます。
error_all_blocked_by_robots = robots.txt がすべての URL をブロックしているため、監査対象がありません。自分のサイトであれば、--ignore-robots を付けて再試行できます。
error_all_excluded_by_pattern = --exclude/--include のパターンによってすべての URL がスキップされたため、監査対象がありません。パターンを確認してください。
error_no_urls_fetched = クロールで URL を 1 件も取得できませんでした。対象を確認してください。
error_dead_seed = { $url } に接続できませんでした({ $cause })。URL を確認してください。
cause_dns = ドメインが存在しないか、名前解決できません
cause_tls = TLS 証明書が有効ではありません
cause_timeout = サーバーが時間内に応答しませんでした
cause_connection = 接続が拒否されました
cause_no_response = 応答なし
warn_base_mismatch = 警告: 正規 URL { $total } 件のうち { $n } 件が、指定したベース({ $base })ではなく { $origin } を指しています。
    インデックス可否の結果はこの指定に左右されます。サイトが
    { $origin } で公開されている場合は、--base { $origin } を付けて監査をやり直してください

## Ficha de una URL (`inspect.rs`)

inspect_error_not_found = { $url } はこのクロールに含まれていません。
inspect_closest = ファイル内で最も近い URL:
inspect_status_title = ステータス
label_http_status = HTTP ステータス
label_content_type = Content-Type
label_response_time = 応答時間
label_depth = 階層
note_depth_clicks = 開始 URL からのクリック数
label_in_sitemap = サイトマップ
word_yes = はい
word_no = いいえ
label_crawl_state = クロール状態
state_note_pending = 検出済みだが、一度も取得されていない
state_note_excluded = クロールから除外({ $reason })
state_note_external_checked = 外部 URL — ステータスは確認済み、内容は未監査
state_note_external_not_checked = 外部 URL — ステータス未確認({ $reason })
state_note_external_unchecked = 外部 URL — 未リクエスト
state_note_out_of_scope = 検出済みだが、このクロールの対象範囲外
state_note_error = リクエストに失敗({ $detail })
region_unknown = 不明
inspect_page_title = ページ
label_page_title = タイトル
label_meta_description = meta description
label_h1 = H1
label_word_count = 単語数
label_canonical = 正規 URL
canonical_self = 自己参照
label_indexable = インデックス可能
indexable_no = いいえ — { $reason }
chars_suffix = { $n } 文字
inspect_redirects_title = リダイレクトチェーン
inlinks_title = 被リンク({ $n })
inlinks_by_region = 領域別: { $breakdown } · nofollow { $nofollow }
inlinks_none = クロールしたページの中に、この URL へリンクしているページはありません。
inlinks_sample_heading = リンク元ページ(本文中のリンクを優先):
no_anchor = (アンカーテキストなし)
inspect_truncated = … { $n } 件を表示 — 全件: { $cmd }
images_title = 画像({ $n })
image_no_alt = (alt なし)
image_usage_title = 画像としての使用状況
# Outbound links header. The three variables are counts.
outlinks_title = 発リンク({ $total }: 内部 { $internal }、外部 { $external })
# Suffix that keeps external URLs apart from the status-code count.
note_external_status = +外部 { $n }
image_usage_line = { $pages } ページで { $times } 回埋め込み

## Comparación de rastreos (`diff.rs`)

diff_title = クロールの比較
label_before = 比較前
label_after = 比較後
label_site = サイト
label_common = 共通
diff_crawl_counts = { $urls } URL、問題 { $findings } 件
diff_common_note = 両方のクロールに存在する URL { $n } 件
warnings_title = 警告
tag_inconclusive = 判定不能
tag_warning = 警告
side_before = 比較前
side_after = 比較後
warn_truncated = 「{ $side }」のクロールは打ち切られています。消えたものや解消されたものは断定できません。単にクロールがそこまで到達しなかった可能性があります。
warn_truncated_by = 「{ $side }」のクロールは { $reason } により打ち切られています。消えたものや解消されたものは断定できません。単にクロールがそこまで到達しなかった可能性があります。
warn_list_mode = 「{ $side }」のクロールはリストクロールで、指定された URL しか見ていません。消えたものや解消されたものは断定できません。
warn_unfinished = 「{ $side }」のクロールは完了していません(ステータス「{ $status }」)。このクロールに含まれないことは、何の意味も持ちません。
warn_rules_changed = ルールカタログが変わっています({ $before } → { $after })。新規または解消された問題の一部は、サイトではなくルールの変更によるものかもしれません。
warn_config_changed = クロール設定が次の項目で変わっています: { $fields }。これを考慮しないと、対象範囲の変化がサイトの変化と見なされてしまいます。
warn_scope_changed = ベース URL が異なります: { $before } → { $after }。同じサイトでも開始地点が違います。
warn_mode_changed = クロールモードが異なります: { $before } → { $after }。
warn_missing_table = 「{ $side }」のクロールは `{ $table }` テーブルより前のものです。この比較はスキップします。
warn_order_inverted = 「比較後」として指定したクロール({ $after_started })は、「比較前」として指定したもの({ $before_started })より先に開始されています。意図したものでなければ、2 つの引数を入れ替えてください。「悪化」と「改善」が逆になります。
worse_title = 悪化
label_new_findings = 新しい問題
more_examples = … ほか { $n } 件
label_pages_lost_index = インデックス不可になったページ
label_status_worse = 悪化したステータスコード
robots_content_changed = { $host }: 内容が変更されました
robots_blocks_all = 注意: サイト全体のクロールをブロックするようになりました。
nothing_worse = ありません。新しい問題も、インデックスから外れたページもありません。
better_title = 改善
label_findings_resolved = 解消された問題
label_pages_indexable_again = 再びインデックス可能になったページ
label_status_better = 改善したステータスコード
other_title = その他の変更
label_new_urls = 新しい URL
label_urls_gone = 消えた URL
label_titles_changed = 変更されたタイトル
label_meta_changed = 変更された meta description
label_canonicals_changed = 変更された正規 URL
label_findings_persisted = 残っている問題
suppressed_title = 断定できない変化
# Multilínea: el punto de llamada imprime cada línea con su sangría.
suppressed_intro = 2 つのクロールの一方が不完全なため、これらの差分はサイトの変化ではなく、
    クロールされなかった部分によるものかもしれません。変更としては数えていません:
label_candidate_new_urls = 新しい URL の候補
label_candidate_urls_gone = 消えた URL の候補
label_candidate_new_findings = 新しい問題の候補
label_candidate_resolved = 解消された問題の候補
suppressed_advice = 確実な比較のため、両方を同じ上限で再クロールしてください。
gate_title = CI ゲート
# Multilínea: el punto de llamada imprime cada línea con su sangría.
gate_inconclusive = 「比較前」のクロールが不完全なため、新しい問題を断定できず、
    --fail-on は判定を行いません。これは PASS ではありません。
gate_fail_detail = 新しい問題 { $n } 件 [{ $severity }] — --fail-on { $token } に該当
gate_pass = PASS   監視対象に新しい問題はありません。

## Reabrir un diff guardado y listar sus filas (`diff_file.rs`)

label_saved = 保存日時
diff_saved_note = 比較の生成: { $generated_at }、crawlforge { $version } · { $path }
# Multilínea: el punto de llamada imprime cada línea con su sangría.
hint_diff_full_lists = 上のリストは一部の例のみです。すべての行を見るには、同じコマンドに --rule <ID> または
    --change <type> を追加してください(例: --change url_removed)。
warn_unknown_change_types = この crawlforge が認識できない種類の変更 { $n } 件({ $types })を除外しました。ファイルは新しいバージョンで書き出されています。
label_status_changed = 変更されたステータスコード
diff_no_matching_changes = この比較には該当する変更がありません。
error_diff_missing = { $path } は存在しません。比較ファイルは `crawlforge diff <before> <after> --out <file>` で保存されます。
error_diff_needs_two = { $path } はクロールファイルです。比較には、以前のクロールと後のクロールの 2 つが必要です。1 つのファイルを指定できるのは、--out で保存した比較ファイルの場合だけです。
hint_diff_prev = 以前のクロールが隣にあります:  crawlforge diff { $prev } { $path }
error_diff_unknown_file = { $path } はクロールファイルでも比較ファイルでもありません。比較ファイルは `crawlforge diff <before> <after> --out <file>` で保存されます。
error_diff_too_new = { $path } は形式 { $version } の比較ファイルで、この crawlforge が対応する形式({ $supported })より新しいものです。開くには crawlforge を更新してください。
error_diff_reopen_flag = { $flag } は 2 つのクロールを比較するときに使います。保存済みの比較は読み込むだけです。クロールに対して実行してください: crawlforge diff <before> <after> { $flag } …
error_unknown_change_type = { $value } は変更の種類ではありません。利用可能: { $available }。
error_rule_with_change = --rule は問題を対象にするため、--change issue_appeared または issue_resolved とのみ組み合わせられます。{ $change } とは組み合わせられません。
site_wide_finding = (サイト全体の問題)
site_placeholder = (サイト)
error_missing_file = { $path } は存在しません
error_store_missing = { $path } は存在しません。クロールファイルは `crawlforge crawl`、`crawlforge audit`、`crawlforge list` で作成されます。
error_not_a_crawl = { $path } は CrawlForge のクロールファイルではないようです
error_different_sites = 2 つのクロールは別のサイトのものです: { $a } と { $b }。
    URL は文字列で照合されるため、比較するとサイト全体が消えて別のサイトが現れたことになってしまいます。各サイトはそれ自身と比較してください。
error_fail_on_unknown = --fail-on: 「{ $token }」はルールでも重要度でもありません。
    重要度は critical、high、medium、low、info です。ルール ID は `crawlforge rules` で表示できます。

## Panel de cartera (`portfolio.rs`)

portfolio_title = ポートフォリオパネル
portfolio_range = { $oldest } ～ { $newest } のクロール
# Trap 3.3: crawls weeks apart are not a snapshot of the portfolio, and "what
# changed" would silently mean a different period on every site.
warn_portfolio_date_spread = 最も古いクロール({ $oldest })と最も新しいクロール({ $newest })は { $days } 日離れています。このパネルはポートフォリオの同時点のスナップショットではなく、「変更点」はサイトごとに異なる期間を対象にしています。
# Trap 3.2: two files crawled with different catalogs are not comparable without
# saying so — a rule can be "missing" on a site because it did not exist yet.
warn_portfolio_rules_versions = すべてのサイトが同じルールカタログでクロールされたわけではありません({ $versions })。そのサイトをクロールした時点でルールが存在しなかったため、結果に現れないことがあります。
warn_portfolio_core_versions = クロールごとにエンジンのバージョンが異なります({ $versions })。
portfolio_skipped_title = 除外したファイル
# A .prev.sqlite is never an input: it is the "before" of the crawl next to it.
portfolio_prev_not_input = .prev.sqlite です。隣にあるクロールの「比較前」で、比較ですでに使用しています。サイトとしては数えません。
error_portfolio_newer_schema = スキーマ(v{ $version })が、このビルドが対応するもの(v{ $supported })より新しいです。開くには crawlforge を更新してください。
error_portfolio_no_sites = 指定したパスに、ポートフォリオとして使えるクロールファイルが見つかりません。クロールファイルは `crawlforge crawl`、`crawlforge audit`、`crawlforge list` で作成されます。ディレクトリを指定すると *.sqlite ファイルを探します。
error_portfolio_tier = ポートフォリオパネルは無料プランに含まれず、Pro が必要です。単一サイト向けのコマンド(`report`、`diff`、`export`)は引き続き使えます。
error_portfolio_too_many = ポートフォリオには { $n } サイトありますが、このプランの上限は { $max } サイトです。
portfolio_changes_title = 変更点
portfolio_no_pairs = 以前のクロールが隣にあるサイトはありません。同じ出力ファイルに再クロールすると、以前のクロールが .prev.sqlite として保持されます。このセクションはそれを「比較前」として比較します。
portfolio_new_critical_high = 重大・高の新しい問題
portfolio_none_critical_high = 比較したどのサイトにも、重大・高の新しい問題はありません。
portfolio_rest_title = その他(サイト別)
portfolio_no_changes = 変更なし
# The diff of that pair cannot assert absences: one side is incomplete.
portfolio_pair_inconclusive = 一方が不完全なため、消えたものや解消されたものは断定できません
portfolio_pair_failed = 以前のクロールとの比較に失敗しました: { $reason }
portfolio_spread_title = ポートフォリオ全体で発生している問題
portfolio_spread_intro = 大半のサイトで検出されるルールの原因が、コンテンツであることはまれです。たいていは共通のテンプレートやプラグインで、1 回の修正ですべてのサイトに効きます。
portfolio_spread_none = ポートフォリオのどのサイトでも検出されたルールはありません。
portfolio_glance_title = ポートフォリオの概要
th_site = サイト
th_crawled = クロール
th_indexable = index可
flag_truncated = (打ち切り)
flag_list_mode = (リストクロール)
flag_unfinished = (未完了: { $status })
hint_site_diff = 1 サイトの比較の全体:
portfolio_html_title = ポートフォリオパネル
portfolio_sites_count = { $n } サイト

## Per-site rule configuration (`rule_config.rs`)

rules_config_source_stored = クロールファイルに保存
rules_config_source_after = 後のクロールに保存
rules_config_source_file = { $path } から読み込み
warn_rules_config_differs = 2 つのクロールに保存されたルール設定が異なります。両方とも同じ設定({ $source })で評価したため、設定の変更が新規または解消された問題として表れることはありません。
warn_portfolio_rules_config = 一部のサイトは独自のルール設定(ルールの無効化／重要度の変更)で読み込まれています: { $sites }。これらの件数は、ルールごとに他のサイトと比較できません。
# "Rule configuration (stored in the crawl file): 2 rules disabled, 1 severity changed."
rules_config_note = ルール設定({ $source }): { $disabled } 件のルールを無効化、{ $changed } 件の重要度を変更。クロールファイルにはすべての問題が保持されています。
# "9 of 12 sites": the numerator of the spread table. The noun follows $total.
portfolio_sites_of = { $total } サイト中 { $fired }
# " (2 inconclusive)": sites where the rule could not be evaluated. Starts with a space.
# Never shown for zero.
portfolio_inconclusive_suffix = {" "}(判定不能 { $n })
# The noun follows $total and the verb follows $pairs: "1 of 5 sites has".
portfolio_pairs_line = { $total } サイト中 { $pairs } サイトに、比較対象となる以前のクロール(.prev.sqlite)があります。

## Identificación de ficheros (`store_check.rs`)

error_store_is_diff = { $path } はクロールファイルではなく、`crawlforge diff --out` が保存する比較ファイルです。`crawlforge diff { $path }` で開いてください。
    クロールファイルは `crawlforge crawl` と `crawlforge audit` で作成されます。
error_store_foreign = { $path } は CrawlForge のクロールファイルではありません。必要なテーブルがありません。別のプログラムのファイルではありませんか？
    クロールファイルは `crawlforge crawl` と `crawlforge audit` で作成されます。
error_store_not_sqlite = { $path } は CrawlForge のファイルではありません。SQLite データベースですらありません。
    クロールファイルは `crawlforge crawl` と `crawlforge audit` で作成されます。

## Informe Markdown/HTML (`audit_report.rs`)

report_audit_of = 監査対象:
report_audit_of_sites = { $n } サイトの監査: { $sites }
sites_and_more = { $sites } ほか { $n } 件
report_mode_line = モード `{ $mode }` · 開始 { $started }
report_truncated_note = > **打ち切られたクロール**(`{ $reason }`)。件数はその時点までにクロールした範囲のもので、サイト全体ではありません。
report_list_mode_note = > **リストクロール。** リストにある URL のみを監査しました。件数はサイト全体ではなくそのリストのもので、サイト全体のリンク構造を必要とするルールは評価されていません。
th_metric = 指標
th_value = 値
row_pages = ページ
row_indexable = インデックス可能
row_errors_4xx_5xx = 4xx/5xx
report_more = … ほか { $n } 件
report_more_run = … ほか { $n } 件 — 実行: { $cmd }
report_full_list_run = … 全件: { $cmd }
html_title = SEO 監査

## Catálogo de reglas (`rules.rs`)

th_severity = 重要度
th_category = カテゴリ
th_scope = 範囲
th_tier = プラン
th_name = 名前
lbl_category = カテゴリ
lbl_scope = 範囲
lbl_tier = プラン
lbl_reference = 参考
scope_page = ページ
scope_site = サイト
rules_summary = ルール { $total } 件: 無料プラン { $free } 件、ページ単位 { $page } 件、サイト全体 { $site } 件。
error_unknown_category = カテゴリ「{ $category }」にルールはありません。カテゴリ: { $list }。

## Cadenas de `main.rs`, listas para cablear

crawling = クロール中: { $target }
error_invalid_seed = { $url } はクロールできる URL ではありません。https://example.com/ のような形式で指定してください。何も作成していません。
file_line = ファイル: { $path }
previous_kept = 前回分:   { $path }(前回の実行分を保持)
report_written = レポートを書き出しました: { $path }
markdown_report_written = Markdown レポートを書き出しました: { $path }
exported_csv = { $n } 個の CSV ファイルを { $dir } に書き出しました
exported_sheets = { $n } 枚のシートを { $path } に書き出しました
bench_appended = ベンチマーク記録を { $path } に追記しました
next_steps_title = 次のステップ
next_explain = すべての問題を該当 URL とともに説明:
next_spreadsheet = 全データをスプレッドシートで開く:
next_compare_previous = このクロールの前回の実行と比較:
next_compare_later = 次回のデプロイ後に同じクロールを再実行し、2 つを比較:
next_reread = この概要をいつでも再表示:

## Autenticación básica HTTP (staging protegido)

auth_url_note = URL に含まれる HTTP Basic 認証を使用します。認証情報は { $host } にのみ送信され、クロールファイルには保存されません。
auth_env_note = CRAWLFORGE_AUTH の HTTP Basic 認証を使用します。認証情報は { $host } にのみ送信され、クロールファイルには保存されません。
auth_base_ignored = 警告: --base に含まれる認証情報を削除しました。ディレクトリの監査はディスクから読み込み、HTTP リクエストを送りません。
auth_list_ignored = 警告: URL リストの各行に含まれる認証情報は無視され、削除されます。クロールに認証を使うには CRAWLFORGE_AUTH=user:password を設定してください。最初の URL のホストにのみ適用されます。
progress_sitemaps = サイトマップを検出中…
progress_finalize = 最終処理: 被リンクとサイト全体のルール…
progress_crawl = { $fetched } 件クロール済み · { $queued } 件待機中 · { $rate } URL/秒 · 問題 { $findings } 件
progress_failed_suffix = {" "}· { $n } 件失敗

## Reanudación de un rastreo interrumpido (`main.rs`, `report.rs`)

resuming = クロールを再開: { $target }
resume_counts = 中断したところから再開します: { $done } URL はクロール済み、{ $pending } URL が未処理です。
resume_config_note = ファイルに保存された設定を使用します。最初の実行時の設定が適用されます。
error_resume_schema_blocking = { $file } はスキーマ v{ $version } のクロールで、マイグレーション { $blocking } はエンジンが書き込む内容を変更します。再開すると、ファイルの半分と残り半分で内容が食い違ってしまいます。クロールをやり直してください。このファイルは引き続き `report`、`export`、`diff` で開けます。
resume_robots_not_inherited = 注意: 元のクロールは robots.txt を無視していましたが、今回は無視しません。この許可はファイルではなく、あなたが与えるものです。必要であれば、元のクロールコマンドを再実行してください。
resume_finished = クロールを再開し、完了しました: 追加で { $urls } URL、{ $secs } 秒
interrupt_flushing = 中断しました。クロール済みの内容を保存しています…(待たずに終了するにはもう一度 Ctrl+C)
interrupt_after_done = 中断しました。クロールファイルは完全です。書き出し途中のエクスポートが残っている可能性があります。
warn_wal_kept = 注意: 別のプログラムが { $path } を開いているため、WAL モードのままになっています。隣にある -wal と -shm ファイルもこのクロールの一部です。3 つまとめてコピーするか、別のプログラムを閉じてから `crawlforge report` を実行して統合してください。
error_store_locked = 別の crawlforge プロセスが現在 { $path } に書き込んでいます。このファイルをクロールまたは再開する前に、処理の終了を待つか、停止してください。
interrupted_saved = クロールを中断しました。ここまでのクロール結果はすべて { $path } に保存されています。続きを実行するには:
hint_previous_unfinished = 前回のクロールは完了していません。最初からやり直さずに続きを実行するには:
error_resume_done = { $path } は完了したクロールです。再開するものはありません。
    サイトを再度監査するには、元のクロールコマンドを実行してください。完了したファイルは .prev.sqlite として隣に保持されます。
error_resume_status = { $path } は再開できません。ステータスが「{ $status }」です。再開できるのは中断したクロール(ステータス「running」または「paused」)だけです。
error_resume_schema = { $path } はスキーマ v{ $found } を使用していますが、このビルドが書き込むのは v{ $expected } です。中断したクロールを再開できるのは、それを開始したバージョンだけです。クロールをやり直してください。
error_resume_config = { $path } に保存されたクロール設定を読み込めません。別のバージョンで書き出されたファイルではありませんか？クロールをやり直してください。
