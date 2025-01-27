use eyre::bail;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Performance::*;

pub fn interpret_pdh_error(value: u32) -> eyre::Result<()> {
    match value {
        x if x == ERROR_SUCCESS.0 => Ok(()),
        PDH_CSTATUS_VALID_DATA => return Ok(()),
        PDH_CSTATUS_NEW_DATA => {
            bail!("The return data value is valid and different from the last sample.");
        }
        PDH_CSTATUS_NO_MACHINE => {
            bail!("Unable to connect to the specified computer, or the computer is offline.");
        }
        PDH_CSTATUS_NO_INSTANCE => {
            bail!("The specified instance is not present.");
        }
        PDH_MORE_DATA => {
            bail!("There is more data to return than would fit in the supplied buffer. Allocate a larger buffer and call the function again.");
        }
        PDH_CSTATUS_ITEM_NOT_VALIDATED => {
            bail!("The data item has been added to the query but has not been validated nor accessed. No other status information on this data item is available.");
        }
        PDH_RETRY => {
            bail!("The selected operation should be retried.");
        }
        PDH_NO_DATA => {
            bail!("No data to return.");
        }
        PDH_CALC_NEGATIVE_DENOMINATOR => {
            bail!("A counter with a negative denominator value was detected.");
        }
        PDH_CALC_NEGATIVE_TIMEBASE => {
            bail!("A counter with a negative time base value was detected.");
        }
        PDH_CALC_NEGATIVE_VALUE => {
            bail!("A counter with a negative value was detected.");
        }
        PDH_DIALOG_CANCELLED => {
            bail!("The user canceled the dialog box.");
        }
        PDH_END_OF_LOG_FILE => {
            bail!("The end of the log file was reached.");
        }
        PDH_ASYNC_QUERY_TIMEOUT => {
            bail!("A time-out occurred while waiting for the asynchronous counter collection thread to end.");
        }
        PDH_CANNOT_SET_DEFAULT_REALTIME_DATASOURCE => {
            bail!("Cannot change set default real-time data source. There are real-time query sessions collecting counter data.");
        }
        PDH_CSTATUS_NO_OBJECT => {
            bail!("The specified object is not found on the system.");
        }
        PDH_CSTATUS_NO_COUNTER => {
            bail!("The specified counter could not be found.");
        }
        PDH_CSTATUS_INVALID_DATA => {
            bail!("The returned data is not valid.");
        }
        PDH_MEMORY_ALLOCATION_FAILURE => {
            bail!("A PDH function could not allocate enough temporary memory to complete the operation. Close some applications or extend the page file and retry the function.");
        }
        PDH_INVALID_HANDLE => {
            bail!("The handle is not a valid PDH object.");
        }
        PDH_INVALID_ARGUMENT => {
            bail!("A required argument is missing or incorrect.");
        }
        PDH_FUNCTION_NOT_FOUND => {
            bail!("Unable to find the specified function.");
        }
        PDH_CSTATUS_NO_COUNTERNAME => {
            bail!("No counter was specified.");
        }
        PDH_CSTATUS_BAD_COUNTERNAME => {
            bail!("Unable to parse the counter path. Check the format and syntax of the specified path.");
        }
        PDH_INVALID_BUFFER => {
            bail!("The buffer passed by the caller is not valid.");
        }
        PDH_INSUFFICIENT_BUFFER => {
            bail!("The requested data is larger than the buffer supplied. Unable to return the requested data.");
        }
        PDH_CANNOT_CONNECT_MACHINE => {
            bail!("Unable to connect to the requested computer.");
        }
        PDH_INVALID_PATH => {
            bail!("The specified counter path could not be interpreted.");
        }
        PDH_INVALID_INSTANCE => {
            bail!("The instance name could not be read from the specified counter path.");
        }
        PDH_INVALID_DATA => {
            bail!("The data is not valid.");
        }
        PDH_NO_DIALOG_DATA => {
            bail!("The dialog box data block was missing or not valid.");
        }
        PDH_CANNOT_READ_NAME_STRINGS => {
            bail!("Unable to read the counter and/or help text from the specified computer.");
        }
        PDH_LOG_FILE_CREATE_ERROR => {
            bail!("Unable to create the specified log file.");
        }
        PDH_LOG_FILE_OPEN_ERROR => {
            bail!("Unable to open the specified log file.");
        }
        PDH_LOG_TYPE_NOT_FOUND => {
            bail!("The specified log file type has not been installed on this system.");
        }
        PDH_NO_MORE_DATA => {
            bail!("No more data is available.");
        }
        PDH_ENTRY_NOT_IN_LOG_FILE => {
            bail!("The specified record was not found in the log file.");
        }
        PDH_DATA_SOURCE_IS_LOG_FILE => {
            bail!("The specified data source is a log file.");
        }
        PDH_DATA_SOURCE_IS_REAL_TIME => {
            bail!("The specified data source is the current activity.");
        }
        PDH_UNABLE_READ_LOG_HEADER => {
            bail!("The log file header could not be read.");
        }
        PDH_FILE_NOT_FOUND => {
            bail!("Unable to find the specified file.");
        }
        PDH_FILE_ALREADY_EXISTS => {
            bail!("There is already a file with the specified file name.");
        }
        PDH_NOT_IMPLEMENTED => {
            bail!("The function referenced has not been implemented.");
        }
        PDH_STRING_NOT_FOUND => {
            bail!("Unable to find the specified string in the list of performance name and help text strings.");
        }
        PDH_UNABLE_MAP_NAME_FILES => {
            bail!("Unable to map to the performance counter name data files. The data will be read from the registry and stored locally.");
        }
        PDH_UNKNOWN_LOG_FORMAT => {
            bail!("The format of the specified log file is not recognized by the PDH DLL.");
        }
        PDH_UNKNOWN_LOGSVC_COMMAND => {
            bail!("The specified Log Service command value is not recognized.");
        }
        PDH_LOGSVC_QUERY_NOT_FOUND => {
            bail!("The specified query from the Log Service could not be found or could not be opened.");
        }
        PDH_LOGSVC_NOT_OPENED => {
            bail!("The Performance Data Log Service key could not be opened. This may be due to insufficient privilege or because the service has not been installed.");
        }
        PDH_WBEM_ERROR => {
            bail!("An error occurred while accessing the WBEM data store.");
        }
        PDH_ACCESS_DENIED => {
            bail!("Unable to access the desired computer or service. Check the permissions and authentication of the log service or the interactive user session against those on the computer or service being monitored.");
        }
        PDH_LOG_FILE_TOO_SMALL => {
            bail!("The maximum log file size specified is too small to log the selected counters. No data will be recorded in this log file. Specify a smaller set of counters to log or a larger file size and retry this call.");
        }
        PDH_INVALID_DATASOURCE => {
            bail!("Cannot connect to ODBC DataSource Name.");
        }
        PDH_INVALID_SQLDB => {
            bail!("SQL Database does not contain a valid set of tables for Perfmon.");
        }
        PDH_NO_COUNTERS => {
            bail!("No counters were found for this Perfmon SQL Log Set.");
        }
        PDH_SQL_ALLOC_FAILED => {
            bail!("Call to SQLAllocStmt failed with %1.");
        }
        PDH_SQL_ALLOCCON_FAILED => {
            bail!("Call to SQLAllocConnect failed with %1.");
        }
        PDH_SQL_EXEC_DIRECT_FAILED => {
            bail!("Call to SQLExecDirect failed with %1.");
        }
        PDH_SQL_FETCH_FAILED => {
            bail!("Call to SQLFetch failed with %1.");
        }
        PDH_SQL_ROWCOUNT_FAILED => {
            bail!("Call to SQLRowCount failed with %1.");
        }
        PDH_SQL_MORE_RESULTS_FAILED => {
            bail!("Call to SQLMoreResults failed with %1.");
        }
        PDH_SQL_CONNECT_FAILED => {
            bail!("Call to SQLConnect failed with %1.");
        }
        PDH_SQL_BIND_FAILED => {
            bail!("Call to SQLBindCol failed with %1.");
        }
        PDH_CANNOT_CONNECT_WMI_SERVER => {
            bail!("Unable to connect to the WMI server on requested computer.");
        }
        PDH_PLA_COLLECTION_ALREADY_RUNNING => {
            bail!("Collection \"%1!s!\" is already running.");
        }
        PDH_PLA_ERROR_SCHEDULE_OVERLAP => {
            bail!("The specified start time is after the end time.");
        }
        PDH_PLA_COLLECTION_NOT_FOUND => {
            bail!("Collection \"%1!s!\" does not exist.");
        }
        PDH_PLA_ERROR_SCHEDULE_ELAPSED => {
            bail!("The specified end time has already elapsed.");
        }
        PDH_PLA_ERROR_NOSTART => {
            bail!("Collection \"%1!s!\" did not start; check the application event log for any errors.");
        }
        PDH_PLA_ERROR_ALREADY_EXISTS => {
            bail!("Collection \"%1!s!\" already exists.");
        }
        PDH_PLA_ERROR_TYPE_MISMATCH => {
            bail!("There is a mismatch in the settings type.");
        }
        PDH_PLA_ERROR_FILEPATH => {
            bail!("The information specified does not resolve to a valid path name.");
        }
        PDH_PLA_SERVICE_ERROR => {
            bail!("The \"Performance Logs & Alerts\" service did not respond.");
        }
        PDH_PLA_VALIDATION_ERROR => {
            bail!("The information passed is not valid.");
        }
        PDH_PLA_VALIDATION_WARNING => {
            bail!("The information passed is not valid.");
        }
        PDH_PLA_ERROR_NAME_TOO_LONG => {
            bail!("The name supplied is too long.");
        }
        PDH_INVALID_SQL_LOG_FORMAT => {
            bail!("SQL log format is incorrect. Correct format is SQL:<DSN-name>!<LogSet-Name>.");
        }
        PDH_COUNTER_ALREADY_IN_QUERY => {
            bail!("Performance counter in PdhAddCounter call has already been added in the performance query. This counter is ignored.");
        }
        PDH_BINARY_LOG_CORRUPT => {
            bail!("Unable to read counter information and data from input binary log files.");
        }
        PDH_LOG_SAMPLE_TOO_SMALL => {
            bail!(
                "At least one of the input binary log files contain fewer than two data samples."
            );
        }
        PDH_OS_LATER_VERSION => {
            bail!("The version of the operating system on the computer named %1 is later than that on the local computer. This operation is not available from the local computer.");
        }
        PDH_OS_EARLIER_VERSION => {
            bail!("%1 supports %2 or later. Check the operating system version on the computer named %3.");
        }
        PDH_INCORRECT_APPEND_TIME => {
            bail!("The output file must contain earlier data than the file to be appended.");
        }
        PDH_UNMATCHED_APPEND_COUNTER => {
            bail!("Both files must have identical counters in order to append.");
        }
        PDH_SQL_ALTER_DETAIL_FAILED => {
            bail!("Cannot alter CounterDetail table layout in SQL database.");
        }
        PDH_QUERY_PERF_DATA_TIMEOUT => {
            bail!("System is busy. A time-out occurred when collecting counter data. Please retry later or increase the CollectTime registry value.");
        }
        x => bail!("Unexpected PDH error: {x:?}"),
    }
}
