@0xabcdef1234567890;

enum Action {
  fetch @0;
  push @1;
}

struct Request {
  requestId @0 :UInt32;
  action @1 :Action;
  resourceUri @2 :Text;
}

struct Response {
  requestId @0 :UInt32;
  statusCode @1 :UInt16;
}