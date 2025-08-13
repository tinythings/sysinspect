# \MinionsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**query_handler**](MinionsApi.md#query_handler) | **POST** /api/v1/query | 
[**query_handler_dev**](MinionsApi.md#query_handler_dev) | **POST** /api/v1/dev_query | 



## query_handler

> models::QueryResponse query_handler(query_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**query_request** | [**QueryRequest**](QueryRequest.md) |  | [required] |

### Return type

[**models::QueryResponse**](QueryResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## query_handler_dev

> models::QueryResponse query_handler_dev(query_payload_request)


Development endpoint for querying minions. FOR DEVELOPMENT AND DEBUGGING PURPOSES ONLY!

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**query_payload_request** | [**QueryPayloadRequest**](QueryPayloadRequest.md) |  | [required] |

### Return type

[**models::QueryResponse**](QueryResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

