# \RsaKeysApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**master_key**](RsaKeysApi.md#master_key) | **POST** /api/v1/masterkey | 
[**pushkey**](RsaKeysApi.md#pushkey) | **POST** /api/v1/pushkey | 



## master_key

> models::MasterKeyResponse master_key()


Retrieve the master public key from the keystore.

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::MasterKeyResponse**](MasterKeyResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## pushkey

> models::PubKeyResponse pushkey(pub_key_request)


Push a public key for a user. Requires an authenticated session ID.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**pub_key_request** | [**PubKeyRequest**](PubKeyRequest.md) |  | [required] |

### Return type

[**models::PubKeyResponse**](PubKeyResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

