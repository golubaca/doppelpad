#pragma once

//
// Define the tracing flags.
//
// Tracing GUID - c6614e05-005f-4c6b-b8c3-fcdc20d31313
//

#define WPP_CONTROL_GUIDS                                              \
    WPP_DEFINE_CONTROL_GUID(                                           \
        DoppelPadTraceGuid, (c6614e05,005f,4c6b,b8c3,fcdc20d31313),    \
                                                                       \
        WPP_DEFINE_BIT(TRACE_DRIVER)                                   \
        WPP_DEFINE_BIT(TRACE_DEVICE)                                   \
        WPP_DEFINE_BIT(TRACE_EVENT)                                    \
        WPP_DEFINE_BIT(TRACE_PARSING)                                  \
        )

#define WPP_FLAG_LEVEL_LOGGER(flag, level)                             \
    WPP_LEVEL_LOGGER(flag)

#define WPP_FLAG_LEVEL_ENABLED(flag, level)                            \
    (WPP_LEVEL_ENABLED(flag) &&                                        \
     WPP_CONTROL(WPP_BIT_ ## flag).Level >= level)

#define WPP_LEVEL_FLAGS_LOGGER(lvl,flags)                              \
           WPP_LEVEL_LOGGER(flags)

#define WPP_LEVEL_FLAGS_ENABLED(lvl, flags)                            \
           (WPP_LEVEL_ENABLED(flags) && WPP_CONTROL(WPP_BIT_ ## flags).Level >= lvl)

//
// This comment block is scanned by the trace preprocessor to define our
// Trace function.
//
// begin_wpp config
// FUNC TraceEvents(LEVEL, FLAGS, MSG, ...);
// end_wpp

//
//
// Driver specific #defines
//
#if UMDF_VERSION_MAJOR == 2 && UMDF_VERSION_MINOR == 0
    #define MYDRIVER_TRACING_ID      L"Microsoft\\UMDF2.0\\DoppelPadDriver V1.0"
#endif
