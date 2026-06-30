# MCP smoke check results

Run date: 2026-06-29T19:57:22.210Z

Checked: 22
Passed: 22
Failed: 0
Timeout per server: 20000ms

Scope: catalog-backed stdio servers with no required secrets and a non-empty command.

| ID | Status | Detail | Duration | Tools |
|---|---|---|---:|---|
| `beads` | pass | 15 tools | 1027ms | discover_tools, get_tool_info, context, ready, list, show, create, claim, update, close, reopen, dep, stats, blocked, admin |
| `blender-mcp` | pass | 22 tools | 581ms | get_scene_info, get_object_info, get_viewport_screenshot, execute_blender_code, get_polyhaven_categories, search_polyhaven_assets, download_polyhaven_asset, set_texture, get_polyhaven_status, get_hyper3d_status, get_sketchfab_status, search_sketchfab_models, get_sketchfab_model_preview, download_sketchfab_model, generate_hyper3d_model_via_text, generate_hyper3d_model_via_images, poll_rodin_job_status, import_generated_asset, get_hunyuan3d_status, generate_hunyuan3d_model |
| `chrome-devtools-mcp` | pass | 29 tools | 889ms | click, close_page, drag, emulate, evaluate_script, fill, fill_form, get_console_message, get_network_request, handle_dialog, hover, lighthouse_audit, list_console_messages, list_network_requests, list_pages, navigate_page, new_page, performance_analyze_insight, performance_start_trace, performance_stop_trace |
| `container-use` | pass | 2 tools | 5356ms | search_container_use, query_docs_filesystem_container_use |
| `context7` | pass | 2 tools | 821ms | resolve-library-id, query-docs |
| `council-of-mine` | pass | 6 tools | 1090ms | start_council_debate, conduct_voting, get_results, list_past_debates, view_debate, get_current_debate_status |
| `fetch` | pass | 1 tools | 557ms | fetch |
| `gitmcp` | pass | 5 tools | 2927ms | match_common_libs_owner_repo_mapping, fetch_generic_documentation, search_generic_documentation, search_generic_code, fetch_generic_url_content |
| `goose-docs` | pass | 4 tools | 3036ms | fetch_goose_documentation, search_goose_documentation, search_goose_code, fetch_generic_url_content |
| `gotoHuman-mcp` | pass | 3 tools | 990ms | list-forms, get-form-schema, request-human-review-with-form |
| `knowledge_graph_memory` | pass | 9 tools | 550ms | create_entities, create_relations, add_observations, delete_entities, delete_observations, delete_relations, read_graph, search_nodes, open_nodes |
| `linux-mcp-server` | pass | 19 tools | 2195ms | get_journal_logs, read_log_file, get_network_interfaces, get_network_connections, get_listening_ports, list_processes, get_process_info, list_services, get_service_status, get_service_logs, list_block_devices, list_directories, list_files, read_file, get_system_information, get_cpu_information, get_memory_information, get_disk_usage, get_hardware_information |
| `mongodb` | pass | 25 tools | 993ms | aggregate-db, aggregate, collection-indexes, collection-schema, collection-storage-size, connect, count, create-collection, create-index, db-stats, delete-many, drop-collection, drop-database, drop-index, explain, export, find, insert-many, list-collections, list-databases |
| `nostrbook-mcp` | pass | 7 tools | 868ms | read_nip, fetch_event, read_kind, read_tag, read_protocol, read_nips_index, generate_kind |
| `pdf_read` | pass | 1 tools | 543ms | read_pdf |
| `pieces` | pass | 2 tools | 2865ms | ask_pieces_ltm, create_pieces_memory |
| `playwright` | pass | 23 tools | 733ms | browser_close, browser_resize, browser_console_messages, browser_handle_dialog, browser_evaluate, browser_file_upload, browser_drop, browser_fill_form, browser_press_key, browser_type, browser_navigate, browser_navigate_back, browser_network_requests, browser_network_request, browser_run_code_unsafe, browser_take_screenshot, browser_snapshot, browser_click, browser_drag, browser_hover |
| `prompts-chat-mcp` | pass | 2 tools | 877ms | search_prompts, get_prompt |
| `repomix-mcp` | pass | 2 tools | 588ms | gather_context, analyze_project |
| `selenium-mcp` | pass | 18 tools | 602ms | start_browser, navigate, interact, send_keys, get_element_text, press_key, upload_file, take_screenshot, close_session, get_element_attribute, execute_script, window, frame, alert, add_cookie, get_cookies, delete_cookie, diagnostics |
| `youtube-transcript-mcp` | pass | 4 tools | 1029ms | get_transcript, get_timed_transcript, get_video_info, get_available_languages |
| `scholar-sidekick` | pass | 6 tools | 805ms | formatCitation, exportCitation, resolveIdentifier, checkRetraction, checkOpenAccess, verifyCitation |

## Failures

