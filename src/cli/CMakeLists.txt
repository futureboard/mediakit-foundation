add_executable(mkff_cli
    main.c
    cli_common.c
    cmd_devices.c
    cmd_va_info.c
    cmd_decode_test.c
    cmd_export_test.c
    cmd_benchmark.c
)

set_target_properties(mkff_cli PROPERTIES OUTPUT_NAME mkff)

target_include_directories(mkff_cli PRIVATE ${PROJECT_SOURCE_DIR}/include)
target_link_libraries(mkff_cli PRIVATE mkff)

install(TARGETS mkff_cli RUNTIME DESTINATION bin)
